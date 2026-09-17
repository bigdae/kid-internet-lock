use crate::lang::{self, WEEKDAY_FULL};
use crate::scheduler::SharedAppState;
use crate::ui::font::{create_ui_font, set_control_fonts};
use crate::ui::ime::disable_ime;
use std::mem::zeroed;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{COLOR_BTNFACE, HBRUSH};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow,
    GetSystemMetrics, GetWindowLongPtrW, GetWindowTextLengthW,
    GetWindowTextW, IsWindow, KillTimer, MessageBoxW,
    RegisterClassExW, SendMessageW, SetForegroundWindow,
    SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_GROUPBOX,
    BS_PUSHBUTTON, CS_HREDRAW, CS_VREDRAW, EN_SETFOCUS, ES_AUTOHSCROLL, ES_CENTER, ES_NUMBER,
    ES_PASSWORD, GWLP_USERDATA, IDC_ARROW, MB_ICONERROR, MB_ICONINFORMATION,
    MB_OK, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE, SW_SHOW, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WM_TIMER, WNDCLASSEXW,
    WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_OVERLAPPED,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

const ID_TIMER_REFRESH: usize = 1;

// Control IDs
const ID_LABEL_STATUS: usize = 205;
const ID_BTN_TEMP_30: usize = 206;
const ID_BTN_TEMP_60: usize = 207;
const ID_BTN_TEMP_CANCEL: usize = 208;
const ID_BTN_TOGGLE_NOW: usize = 209;

const ID_EDIT_OLD_PWD: usize = 210;
const ID_EDIT_NEW_PWD: usize = 211;
const ID_EDIT_CONFIRM_PWD: usize = 212;
const ID_BTN_CHANGE_PWD: usize = 213;
const ID_LABEL_PWD_MSG: usize = 214;

const ID_CHK_AUTOSTART: usize = 215;

const ID_BTN_SAVE: usize = 216;
const ID_BTN_CLOSE: usize = 217;

// Per-weekday schedule controls: Monday = 0 .. Sunday = 6.
// Each day has two block slots; slot 2 is unused when start equals end.
const ID_CHK_DAY_BASE: usize = 220;
const ID_EDIT_START_H_BASE: usize = 230;
const ID_EDIT_START_M_BASE: usize = 240;
const ID_EDIT_END_H_BASE: usize = 250;
const ID_EDIT_END_M_BASE: usize = 260;
const ID_EDIT_START2_H_BASE: usize = 270;
const ID_EDIT_START2_M_BASE: usize = 280;
const ID_EDIT_END2_H_BASE: usize = 290;
const ID_EDIT_END2_M_BASE: usize = 300;

// Batch input: slot 1 edits 310-313, slot 2 edits 314-317 (order: start_h, start_m, end_h, end_m).
const ID_BATCH1_BASE: usize = 310;
const ID_BATCH2_BASE: usize = 314;
const ID_BTN_APPLY_SLOT1: usize = 318;
const ID_BTN_APPLY_SLOT2: usize = 319;

const EN_KILLFOCUS: u32 = 0x0200;
const EM_SETLIMITTEXT: u32 = 0x00C5;

static mut ACTIVE_SETTINGS_HWND: HWND = null_mut();

struct SettingsContext {
    shared_state: SharedAppState,
    chk_days: [HWND; 7],
    edit_start_h: [HWND; 7],
    edit_start_m: [HWND; 7],
    edit_end_h: [HWND; 7],
    edit_end_m: [HWND; 7],
    edit_start2_h: [HWND; 7],
    edit_start2_m: [HWND; 7],
    edit_end2_h: [HWND; 7],
    edit_end2_m: [HWND; 7],
    batch1: [HWND; 4],
    batch2: [HWND; 4],
    label_status: HWND,
    edit_old_pwd: HWND,
    edit_new_pwd: HWND,
    edit_confirm_pwd: HWND,
    label_pwd_msg: HWND,
    chk_autostart: HWND,
}

unsafe fn parse_hm(h_hwnd: HWND, m_hwnd: HWND) -> Option<(u32, u32)> {
    unsafe {
        let h = read_edit_text(h_hwnd).parse::<u32>().ok()?;
        let m = read_edit_text(m_hwnd).parse::<u32>().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        Some((h, m))
    }
}

unsafe fn read_edit_text(hwnd: HWND) -> String {
    unsafe {
        // Cap length so overlong/pasted input can never exhaust memory or panic.
        let len = (GetWindowTextLengthW(hwnd) as usize).min(1024);
        let mut buf = vec![0u16; len + 1];
        GetWindowTextW(hwnd, buf.as_mut_ptr(), (len + 1) as i32);
        buf.pop();
        String::from_utf16_lossy(&buf).trim().to_string()
    }
}

/// Maps a time-edit control ID to its HWND (per-weekday slots and batch inputs).
fn time_edit_hwnd(ctx: &SettingsContext, id: usize) -> Option<HWND> {
    let day_groups: [(usize, &[HWND; 7]); 8] = [
        (ID_EDIT_START_H_BASE, &ctx.edit_start_h),
        (ID_EDIT_START_M_BASE, &ctx.edit_start_m),
        (ID_EDIT_END_H_BASE, &ctx.edit_end_h),
        (ID_EDIT_END_M_BASE, &ctx.edit_end_m),
        (ID_EDIT_START2_H_BASE, &ctx.edit_start2_h),
        (ID_EDIT_START2_M_BASE, &ctx.edit_start2_m),
        (ID_EDIT_END2_H_BASE, &ctx.edit_end2_h),
        (ID_EDIT_END2_M_BASE, &ctx.edit_end2_m),
    ];
    for (base, arr) in day_groups {
        if id >= base && id < base + 7 {
            return Some(arr[id - base]);
        }
    }
    let batch_groups: [(usize, &[HWND; 4]); 2] =
        [(ID_BATCH1_BASE, &ctx.batch1), (ID_BATCH2_BASE, &ctx.batch2)];
    for (base, arr) in batch_groups {
        if id >= base && id < base + 4 {
            return Some(arr[id - base]);
        }
    }
    None
}

unsafe fn set_edit_text(hwnd: HWND, text: &str) {
    unsafe {
        let wide: Vec<u16> = format!("{}\0", text).encode_utf16().collect();
        SetWindowTextW(hwnd, wide.as_ptr());
    }
}

unsafe fn update_status_text(ctx: &SettingsContext) {
    unsafe {
        let mut state = ctx.shared_state.lock().unwrap();
        let detail = state.evaluate_and_sync();
        let text_wide: Vec<u16> = format!("{}\0", lang::status_line(&detail.summary))
            .encode_utf16()
            .collect();
        SetWindowTextW(ctx.label_status, text_wide.as_ptr());
    }
}

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsContext;

        match msg {
            WM_TIMER => {
                if wparam == ID_TIMER_REFRESH && !ctx_ptr.is_null() {
                    let ctx = &*ctx_ptr;
                    update_status_text(ctx);
                }
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as usize;
                if ctx_ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let ctx = &mut *ctx_ptr;

                let notify = ((wparam >> 16) & 0xFFFF) as u32;
                if notify == EN_SETFOCUS {
                    match id {
                        ID_EDIT_OLD_PWD => disable_ime(ctx.edit_old_pwd),
                        ID_EDIT_NEW_PWD => disable_ime(ctx.edit_new_pwd),
                        ID_EDIT_CONFIRM_PWD => disable_ime(ctx.edit_confirm_pwd),
                        _ => {
                            // Clear "00" so typing a new time needs no manual erase.
                            if let Some(h) = time_edit_hwnd(ctx, id) {
                                if read_edit_text(h) == "00" {
                                    SetWindowTextW(h, [0u16].as_ptr());
                                }
                            }
                        }
                    }
                    return 0;
                }
                if notify == EN_KILLFOCUS {
                    // Restore "00" when left empty; pad a single digit ("5" -> "05").
                    if let Some(h) = time_edit_hwnd(ctx, id) {
                        let t = read_edit_text(h);
                        if t.is_empty() {
                            set_edit_text(h, "00");
                        } else if t.len() == 1 && t.bytes().all(|b| b.is_ascii_digit()) {
                            set_edit_text(h, &format!("0{}", t));
                        }
                    }
                    return 0;
                }

                match id {
                    ID_BTN_TEMP_30 => {
                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            state.set_temporary_allow(30);
                            state.evaluate_and_sync();
                        }
                        update_status_text(ctx);
                        0
                    }
                    ID_BTN_TEMP_60 => {
                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            state.set_temporary_allow(60);
                            state.evaluate_and_sync();
                        }
                        update_status_text(ctx);
                        0
                    }
                    ID_BTN_TEMP_CANCEL => {
                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            state.cancel_temporary_allow();
                            state.clear_manual_override();
                            state.evaluate_and_sync();
                        }
                        update_status_text(ctx);
                        0
                    }
                    ID_BTN_TOGGLE_NOW => {
                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            state.toggle_manual_override();
                            state.evaluate_and_sync();
                        }
                        update_status_text(ctx);
                        0
                    }
                    ID_BTN_CHANGE_PWD => {
                        let old_pwd = read_edit_text(ctx.edit_old_pwd);
                        let new_pwd = read_edit_text(ctx.edit_new_pwd);
                        let confirm_pwd = read_edit_text(ctx.edit_confirm_pwd);

                        let mut state = ctx.shared_state.lock().unwrap();
                        let result_msg = if !state.config.verify_password(&old_pwd) {
                            lang::PWD_ERR_CURRENT
                        } else if new_pwd.is_empty() {
                            lang::PWD_ERR_EMPTY
                        } else if new_pwd != confirm_pwd {
                            lang::PWD_ERR_MISMATCH
                        } else {
                            state.config.update_password(&new_pwd);
                            if let Err(e) = state.config.save() {
                                SetWindowTextW(ctx.label_pwd_msg, format!("{}\0", lang::save_failed_msg(&e)).encode_utf16().collect::<Vec<_>>().as_ptr());
                                return 0;
                            }
                            SetWindowTextW(ctx.edit_old_pwd, [0u16].as_ptr());
                            SetWindowTextW(ctx.edit_new_pwd, [0u16].as_ptr());
                            SetWindowTextW(ctx.edit_confirm_pwd, [0u16].as_ptr());
                            lang::PWD_OK
                        };

                        let msg_wide: Vec<u16> = format!("{}\0", result_msg).encode_utf16().collect();
                        SetWindowTextW(ctx.label_pwd_msg, msg_wide.as_ptr());
                        0
                    }
                    ID_BTN_APPLY_SLOT1 | ID_BTN_APPLY_SLOT2 => {
                        let (batch, slot_no) = if id == ID_BTN_APPLY_SLOT1 {
                            (&ctx.batch1, lang::SLOT1_NAME)
                        } else {
                            (&ctx.batch2, lang::SLOT2_NAME)
                        };
                        let start = parse_hm(batch[0], batch[1]);
                        let end = parse_hm(batch[2], batch[3]);
                        match (start, end) {
                            (Some((sh, sm)), Some((eh, em))) => {
                                let sh_t = format!("{:02}", sh);
                                let sm_t = format!("{:02}", sm);
                                let eh_t = format!("{:02}", eh);
                                let em_t = format!("{:02}", em);
                                let (dst_h, dst_m, dst_eh, dst_em) = if id == ID_BTN_APPLY_SLOT1 {
                                    (&ctx.edit_start_h, &ctx.edit_start_m, &ctx.edit_end_h, &ctx.edit_end_m)
                                } else {
                                    (&ctx.edit_start2_h, &ctx.edit_start2_m, &ctx.edit_end2_h, &ctx.edit_end2_m)
                                };
                                for i in 0..7 {
                                    set_edit_text(dst_h[i], &sh_t);
                                    set_edit_text(dst_m[i], &sm_t);
                                    set_edit_text(dst_eh[i], &eh_t);
                                    set_edit_text(dst_em[i], &em_t);
                                }
                            }
                            _ => {
                                let title: Vec<u16> = format!("{}\0", lang::TITLE_INPUT_ERROR).encode_utf16().collect();
                                let msg: Vec<u16> = format!("{}\0", lang::batch_error(slot_no))
                                .encode_utf16()
                                .collect();
                                MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
                            }
                        }
                        0
                    }
                    ID_BTN_SAVE => {
                        let mut parsed_days = [(false, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32, 0u32); 7];
                        for i in 0..7 {
                            let slot1 = parse_hm(ctx.edit_start_h[i], ctx.edit_start_m[i]);
                            let slot1_end = parse_hm(ctx.edit_end_h[i], ctx.edit_end_m[i]);
                            let slot2 = parse_hm(ctx.edit_start2_h[i], ctx.edit_start2_m[i]);
                            let slot2_end = parse_hm(ctx.edit_end2_h[i], ctx.edit_end2_m[i]);

                            let ((sh, sm), (eh, em), (sh2, sm2), (eh2, em2)) =
                                match (slot1, slot1_end, slot2, slot2_end) {
                                    (Some(s), Some(e), Some(s2), Some(e2)) => (s, e, s2, e2),
                                    _ => {
                                        let title: Vec<u16> = format!("{}\0", lang::TITLE_INPUT_ERROR).encode_utf16().collect();
                                let msg: Vec<u16> = format!("{}\0", lang::weekly_error(WEEKDAY_FULL[i]))
                                        .encode_utf16()
                                        .collect();
                                        MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
                                        return 0;
                                    }
                                };

                            let enabled = SendMessageW(ctx.chk_days[i], BM_GETCHECK, 0, 0) == 1;
                            parsed_days[i] = (enabled, sh, sm, eh, em, sh2, sm2, eh2, em2);
                        }

                        let auto_start = SendMessageW(ctx.chk_autostart, BM_GETCHECK, 0, 0) == 1;

                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            for (i, (enabled, sh, sm, eh, em, sh2, sm2, eh2, em2)) in parsed_days.iter().enumerate() {
                                state.config.weekly_schedule[i].enabled = *enabled;
                                state.config.weekly_schedule[i].start_hour = *sh;
                                state.config.weekly_schedule[i].start_minute = *sm;
                                state.config.weekly_schedule[i].end_hour = *eh;
                                state.config.weekly_schedule[i].end_minute = *em;
                                state.config.weekly_schedule[i].start2_hour = *sh2;
                                state.config.weekly_schedule[i].start2_minute = *sm2;
                                state.config.weekly_schedule[i].end2_hour = *eh2;
                                state.config.weekly_schedule[i].end2_minute = *em2;
                            }
                            state.config.sync_legacy_from_weekly();
                            state.config.auto_start = auto_start;

                            let _ = state.config.save();
                            let _ = state.config.sync_autostart();
                            state.evaluate_and_sync();
                        }

                        let title: Vec<u16> = format!("{}\0", lang::TITLE_SAVED).encode_utf16().collect();
                        let msg: Vec<u16> = format!("{}\0", lang::MSG_SAVED).encode_utf16().collect();
                        MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
                        update_status_text(ctx);
                        0
                    }
                    ID_BTN_CLOSE => {
                        DestroyWindow(hwnd);
                        0
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, ID_TIMER_REFRESH);
                ACTIVE_SETTINGS_HWND = null_mut();
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Opens the Administrator Settings window. If already open, brings it to foreground.
pub fn open_settings_dialog(shared_state: SharedAppState) {
    unsafe {
        if !ACTIVE_SETTINGS_HWND.is_null() && IsWindow(ACTIVE_SETTINGS_HWND) != 0 {
            ShowWindow(ACTIVE_SETTINGS_HWND, SW_RESTORE);
            SetForegroundWindow(ACTIVE_SETTINGS_HWND);
            return;
        }

        let class_name: Vec<u16> = "KidInternetLock_SettingsWnd\0".encode_utf16().collect();
        let title_wide: Vec<u16> = format!("{}\0", lang::TITLE_SETTINGS).encode_utf16().collect();
        let hinstance = GetModuleHandleW(null_mut());

        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(settings_wnd_proc);
        wc.hInstance = hinstance;
        wc.hCursor = windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW(
            null_mut(),
            IDC_ARROW,
        );
        wc.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);

        let width = 760;
        let height = 816;
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - width) / 2;
        let y = (screen_h - height) / 2;

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title_wide.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            width,
            height,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return;
        }

        ACTIVE_SETTINGS_HWND = hwnd;

        let font = create_ui_font(13, false);
        let font_bold = create_ui_font(13, true);

        let static_class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let edit_class: Vec<u16> = "EDIT\0".encode_utf16().collect();
        let btn_class: Vec<u16> = "BUTTON\0".encode_utf16().collect();

        // -------------------------------------------------------------
        // Group 1: Block schedule (per weekday, two slots each)
        // -------------------------------------------------------------
        let grp1_title: Vec<u16> = format!("{}\0", lang::GRP_SCHEDULE).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp1_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            12,
            708,
            326,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        // Batch input rows: enter once, copy to every weekday.
        let mut batch1: [HWND; 4] = [null_mut(); 4];
        let mut batch2: [HWND; 4] = [null_mut(); 4];
        for (row, batch, base, label_txt, btn_id, btn_txt) in [
            (36, &mut batch1, ID_BATCH1_BASE, lang::BATCH1_LABEL, ID_BTN_APPLY_SLOT1, lang::BTN_COPY_SLOT1),
            (64, &mut batch2, ID_BATCH2_BASE, lang::BATCH2_LABEL, ID_BTN_APPLY_SLOT2, lang::BTN_COPY_SLOT2),
        ] {
            let row_y = row;
            let label_wide: Vec<u16> = format!("{}\0", label_txt).encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), label_wide.as_ptr(), WS_CHILD | WS_VISIBLE, 36, row_y + 2, 90, 20, hwnd, null_mut(), hinstance, null_mut());
            let edit_x = [140, 200, 266, 326];
            for (k, x) in edit_x.iter().enumerate() {
                let h = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), *x, row_y, 44, 24, hwnd, (base + k) as _, hinstance, null_mut());
                SendMessageW(h, EM_SETLIMITTEXT, 2, 0);
                batch[k] = h;
            }
            let colon: Vec<u16> = ":\0".encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 188, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 314, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            let dash: Vec<u16> = "~\0".encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), dash.as_ptr(), WS_CHILD | WS_VISIBLE, 250, row_y + 2, 12, 20, hwnd, null_mut(), hinstance, null_mut());
            let btn_wide: Vec<u16> = format!("{}\0", btn_txt).encode_utf16().collect();
            CreateWindowExW(0, btn_class.as_ptr(), btn_wide.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 390, row_y - 1, 180, 26, hwnd, btn_id as _, hinstance, null_mut());
        }

        let hdr_day: Vec<u16> = format!("{}\0", lang::HDR_DAY).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), hdr_day.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 94, 60, 20, hwnd, null_mut(), hinstance, null_mut());
        let hdr_start: Vec<u16> = format!("{}\0", lang::HDR_SLOT1).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), hdr_start.as_ptr(), WS_CHILD | WS_VISIBLE, 150, 94, 200, 20, hwnd, null_mut(), hinstance, null_mut());
        let hdr_end: Vec<u16> = format!("{}\0", lang::HDR_SLOT2).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), hdr_end.as_ptr(), WS_CHILD | WS_VISIBLE, 405, 94, 300, 20, hwnd, null_mut(), hinstance, null_mut());

        let mut chk_days: [HWND; 7] = [null_mut(); 7];
        let mut edit_start_h: [HWND; 7] = [null_mut(); 7];
        let mut edit_start_m: [HWND; 7] = [null_mut(); 7];
        let mut edit_end_h: [HWND; 7] = [null_mut(); 7];
        let mut edit_end_m: [HWND; 7] = [null_mut(); 7];
        let mut edit_start2_h: [HWND; 7] = [null_mut(); 7];
        let mut edit_start2_m: [HWND; 7] = [null_mut(); 7];
        let mut edit_end2_h: [HWND; 7] = [null_mut(); 7];
        let mut edit_end2_m: [HWND; 7] = [null_mut(); 7];

        for i in 0..7 {
            let row_y = 116 + (i as i32) * 26;
            let day_label: Vec<u16> = format!("{}\0", WEEKDAY_FULL[i]).encode_utf16().collect();
            chk_days[i] = CreateWindowExW(
                0,
                btn_class.as_ptr(),
                day_label.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_AUTOCHECKBOX as u32),
                36,
                row_y,
                95,
                24,
                hwnd,
                (ID_CHK_DAY_BASE + i) as _,
                hinstance,
                null_mut(),
            );

            // Slot 1: 150 - 355
            edit_start_h[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 150, row_y, 44, 24, hwnd, (ID_EDIT_START_H_BASE + i) as _, hinstance, null_mut());
            let colon: Vec<u16> = ":\0".encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 198, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            edit_start_m[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 210, row_y, 44, 24, hwnd, (ID_EDIT_START_M_BASE + i) as _, hinstance, null_mut());

            let dash: Vec<u16> = "~\0".encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), dash.as_ptr(), WS_CHILD | WS_VISIBLE, 260, row_y + 2, 14, 20, hwnd, null_mut(), hinstance, null_mut());

            edit_end_h[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 278, row_y, 44, 24, hwnd, (ID_EDIT_END_H_BASE + i) as _, hinstance, null_mut());
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 326, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            edit_end_m[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 338, row_y, 44, 24, hwnd, (ID_EDIT_END_M_BASE + i) as _, hinstance, null_mut());

            let sep: Vec<u16> = "|\0".encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), sep.as_ptr(), WS_CHILD | WS_VISIBLE, 390, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());

            // Slot 2: 405 - 640
            edit_start2_h[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 405, row_y, 44, 24, hwnd, (ID_EDIT_START2_H_BASE + i) as _, hinstance, null_mut());
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 453, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            edit_start2_m[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 465, row_y, 44, 24, hwnd, (ID_EDIT_START2_M_BASE + i) as _, hinstance, null_mut());
            CreateWindowExW(0, static_class.as_ptr(), dash.as_ptr(), WS_CHILD | WS_VISIBLE, 515, row_y + 2, 14, 20, hwnd, null_mut(), hinstance, null_mut());
            edit_end2_h[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 533, row_y, 44, 24, hwnd, (ID_EDIT_END2_H_BASE + i) as _, hinstance, null_mut());
            CreateWindowExW(0, static_class.as_ptr(), colon.as_ptr(), WS_CHILD | WS_VISIBLE, 581, row_y + 2, 10, 20, hwnd, null_mut(), hinstance, null_mut());
            edit_end2_m[i] = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 593, row_y, 44, 24, hwnd, (ID_EDIT_END2_M_BASE + i) as _, hinstance, null_mut());

            let row_hint: Vec<u16> = format!("{}\0", lang::ROW_HINT).encode_utf16().collect();
            CreateWindowExW(0, static_class.as_ptr(), row_hint.as_ptr(), WS_CHILD | WS_VISIBLE, 650, row_y + 2, 60, 20, hwnd, null_mut(), hinstance, null_mut());
        }

        // No more than 2 digits per time field: blocks the 4-5 digit input
        // that used to break saving, at the control level (typing and paste).
        for group in [
            &edit_start_h,
            &edit_start_m,
            &edit_end_h,
            &edit_end_m,
            &edit_start2_h,
            &edit_start2_m,
            &edit_end2_h,
            &edit_end2_m,
        ] {
            for &h in group.iter() {
                SendMessageW(h, EM_SETLIMITTEXT, 2, 0);
            }
        }

        let note_text: Vec<u16> = format!("{}\0", lang::NOTE_SCHEDULE).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), note_text.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 304, 670, 20, hwnd, null_mut(), hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 2: Temporary allow & manual control
        // -------------------------------------------------------------
        let grp2_title: Vec<u16> = format!("{}\0", lang::GRP_TEMP).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp2_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            348,
            708,
            115,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let label_status = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            36,
            373,
            670,
            24,
            hwnd,
            ID_LABEL_STATUS as _,
            hinstance,
            null_mut(),
        );

        let btn_30_txt: Vec<u16> = format!("{}\0", lang::BTN_ALLOW_30).encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_30_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 36, 408, 130, 32, hwnd, ID_BTN_TEMP_30 as _, hinstance, null_mut());

        let btn_60_txt: Vec<u16> = format!("{}\0", lang::BTN_ALLOW_60).encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_60_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 176, 408, 130, 32, hwnd, ID_BTN_TEMP_60 as _, hinstance, null_mut());

        let btn_cancel_txt: Vec<u16> = format!("{}\0", lang::BTN_TEMP_CANCEL).encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_cancel_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 316, 408, 160, 32, hwnd, ID_BTN_TEMP_CANCEL as _, hinstance, null_mut());

        let btn_toggle_txt: Vec<u16> = format!("{}\0", lang::BTN_TOGGLE).encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_toggle_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 486, 408, 160, 32, hwnd, ID_BTN_TOGGLE_NOW as _, hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 3: Change admin password
        // -------------------------------------------------------------
        let grp3_title: Vec<u16> = format!("{}\0", lang::GRP_PASSWORD).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp3_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            473,
            708,
            160,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let lbl_old: Vec<u16> = format!("{}\0", lang::LBL_CURRENT_PWD).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_old.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 503, 150, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_old_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 190, 501, 170, 24, hwnd, ID_EDIT_OLD_PWD as _, hinstance, null_mut());

        let lbl_new: Vec<u16> = format!("{}\0", lang::LBL_NEW_PWD).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_new.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 536, 150, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_new_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 190, 534, 170, 24, hwnd, ID_EDIT_NEW_PWD as _, hinstance, null_mut());

        let lbl_conf: Vec<u16> = format!("{}\0", lang::LBL_CONFIRM_PWD).encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_conf.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 569, 150, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_confirm_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 190, 567, 170, 24, hwnd, ID_EDIT_CONFIRM_PWD as _, hinstance, null_mut());

        disable_ime(edit_old_pwd);
        disable_ime(edit_new_pwd);
        disable_ime(edit_confirm_pwd);

        let btn_pwd_txt: Vec<u16> = format!("{}\0", lang::BTN_CHANGE_PWD).encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_pwd_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 440, 518, 160, 42, hwnd, ID_BTN_CHANGE_PWD as _, hinstance, null_mut());

        let label_pwd_msg = CreateWindowExW(0, static_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE, 36, 602, 660, 20, hwnd, ID_LABEL_PWD_MSG as _, hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 4: Startup settings
        // -------------------------------------------------------------
        let grp4_title: Vec<u16> = format!("{}\0", lang::GRP_STARTUP).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp4_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            643,
            708,
            60,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let chk_txt: Vec<u16> = format!("{}\0", lang::CHK_AUTOSTART).encode_utf16().collect();
        let chk_autostart = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            chk_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_AUTOCHECKBOX as u32),
            36,
            666,
            620,
            24,
            hwnd,
            ID_CHK_AUTOSTART as _,
            hinstance,
            null_mut(),
        );

        // -------------------------------------------------------------
        // Bottom Action Buttons
        // -------------------------------------------------------------
        let save_txt: Vec<u16> = format!("{}\0", lang::BTN_SAVE).encode_utf16().collect();
        let btn_save = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            save_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON as u32),
            496,
            723,
            110,
            36,
            hwnd,
            ID_BTN_SAVE as _,
            hinstance,
            null_mut(),
        );

        let close_txt: Vec<u16> = format!("{}\0", lang::BTN_CLOSE).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            close_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32),
            616,
            723,
            100,
            36,
            hwnd,
            ID_BTN_CLOSE as _,
            hinstance,
            null_mut(),
        );

        // Apply modern UI fonts
        set_control_fonts(hwnd, font);
        SendMessageW(btn_save, windows_sys::Win32::UI::WindowsAndMessaging::WM_SETFONT, font_bold as _, 1);

        // Populate initial values from config (batch rows start from Monday's slots)
        {
            let state = shared_state.lock().unwrap();
            {
                let monday = &state.config.weekly_schedule[0];
                let batch_vals = [
                    monday.start_hour,
                    monday.start_minute,
                    monday.end_hour,
                    monday.end_minute,
                    monday.start2_hour,
                    monday.start2_minute,
                    monday.end2_hour,
                    monday.end2_minute,
                ];
                for (k, v) in batch_vals.iter().enumerate() {
                    let text: Vec<u16> = format!("{:02}\0", v).encode_utf16().collect();
                    let target = if k < 4 { batch1[k] } else { batch2[k - 4] };
                    SetWindowTextW(target, text.as_ptr());
                }
            }
            for i in 0..7 {
                let day = &state.config.weekly_schedule[i];
                let sh_str: Vec<u16> = format!("{:02}\0", day.start_hour).encode_utf16().collect();
                let sm_str: Vec<u16> = format!("{:02}\0", day.start_minute).encode_utf16().collect();
                let eh_str: Vec<u16> = format!("{:02}\0", day.end_hour).encode_utf16().collect();
                let em_str: Vec<u16> = format!("{:02}\0", day.end_minute).encode_utf16().collect();
                let sh2_str: Vec<u16> = format!("{:02}\0", day.start2_hour).encode_utf16().collect();
                let sm2_str: Vec<u16> = format!("{:02}\0", day.start2_minute).encode_utf16().collect();
                let eh2_str: Vec<u16> = format!("{:02}\0", day.end2_hour).encode_utf16().collect();
                let em2_str: Vec<u16> = format!("{:02}\0", day.end2_minute).encode_utf16().collect();

                SetWindowTextW(edit_start_h[i], sh_str.as_ptr());
                SetWindowTextW(edit_start_m[i], sm_str.as_ptr());
                SetWindowTextW(edit_end_h[i], eh_str.as_ptr());
                SetWindowTextW(edit_end_m[i], em_str.as_ptr());
                SetWindowTextW(edit_start2_h[i], sh2_str.as_ptr());
                SetWindowTextW(edit_start2_m[i], sm2_str.as_ptr());
                SetWindowTextW(edit_end2_h[i], eh2_str.as_ptr());
                SetWindowTextW(edit_end2_m[i], em2_str.as_ptr());

                SendMessageW(
                    chk_days[i],
                    BM_SETCHECK,
                    if day.enabled { 1 } else { 0 },
                    0,
                );
            }

            SendMessageW(
                chk_autostart,
                BM_SETCHECK,
                if state.config.auto_start { 1 } else { 0 },
                0,
            );
        }

        let ctx = Box::into_raw(Box::new(SettingsContext {
            shared_state: shared_state.clone(),
            chk_days,
            edit_start_h,
            edit_start_m,
            edit_end_h,
            edit_end_m,
            edit_start2_h,
            edit_start2_m,
            edit_end2_h,
            edit_end2_m,
            batch1,
            batch2,
            label_status,
            edit_old_pwd,
            edit_new_pwd,
            edit_confirm_pwd,
            label_pwd_msg,
            chk_autostart,
        }));

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);

        // Initial status update & start 1-second refresh timer
        update_status_text(&*ctx);
        SetTimer(hwnd, ID_TIMER_REFRESH, 1000, None);

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}
