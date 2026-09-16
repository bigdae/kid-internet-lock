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
const ID_EDIT_START_H: usize = 201;
const ID_EDIT_START_M: usize = 202;
const ID_EDIT_END_H: usize = 203;
const ID_EDIT_END_M: usize = 204;

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

static mut ACTIVE_SETTINGS_HWND: HWND = null_mut();

struct SettingsContext {
    shared_state: SharedAppState,
    edit_start_h: HWND,
    edit_start_m: HWND,
    edit_end_h: HWND,
    edit_end_m: HWND,
    label_status: HWND,
    edit_old_pwd: HWND,
    edit_new_pwd: HWND,
    edit_confirm_pwd: HWND,
    label_pwd_msg: HWND,
    chk_autostart: HWND,
}

unsafe fn read_edit_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd) as usize;
        let mut buf = vec![0u16; len + 1];
        GetWindowTextW(hwnd, buf.as_mut_ptr(), (len + 1) as i32);
        buf.pop();
        String::from_utf16_lossy(&buf).trim().to_string()
    }
}

unsafe fn update_status_text(ctx: &SettingsContext) {
    unsafe {
        let mut state = ctx.shared_state.lock().unwrap();
        let detail = state.evaluate_and_sync();
        let text_wide: Vec<u16> = format!("Current status: {}\0", detail.summary)
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
                        _ => {}
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
                            "The current password is incorrect."
                        } else if new_pwd.is_empty() {
                            "Please enter a new password."
                        } else if new_pwd != confirm_pwd {
                            "The new password confirmation does not match."
                        } else {
                            state.config.update_password(&new_pwd);
                            if let Err(e) = state.config.save() {
                                SetWindowTextW(ctx.label_pwd_msg, format!("Save failed: {}\0", e).encode_utf16().collect::<Vec<_>>().as_ptr());
                                return 0;
                            }
                            SetWindowTextW(ctx.edit_old_pwd, [0u16].as_ptr());
                            SetWindowTextW(ctx.edit_new_pwd, [0u16].as_ptr());
                            SetWindowTextW(ctx.edit_confirm_pwd, [0u16].as_ptr());
                            "Password changed successfully."
                        };

                        let msg_wide: Vec<u16> = format!("{}\0", result_msg).encode_utf16().collect();
                        SetWindowTextW(ctx.label_pwd_msg, msg_wide.as_ptr());
                        0
                    }
                    ID_BTN_SAVE => {
                        let sh_str = read_edit_text(ctx.edit_start_h);
                        let sm_str = read_edit_text(ctx.edit_start_m);
                        let eh_str = read_edit_text(ctx.edit_end_h);
                        let em_str = read_edit_text(ctx.edit_end_m);

                        let sh = sh_str.parse::<u32>().unwrap_or(99);
                        let sm = sm_str.parse::<u32>().unwrap_or(99);
                        let eh = eh_str.parse::<u32>().unwrap_or(99);
                        let em = em_str.parse::<u32>().unwrap_or(99);

                        if sh > 23 || eh > 23 || sm > 59 || em > 59 {
                            let title: Vec<u16> = "Input Error\0".encode_utf16().collect();
                            let msg: Vec<u16> = "Please enter hours between 00 and 23 and minutes between 00 and 59.\0"
                                .encode_utf16()
                                .collect();
                            MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
                            return 0;
                        }

                        let auto_start = SendMessageW(ctx.chk_autostart, BM_GETCHECK, 0, 0) == 1;

                        {
                            let mut state = ctx.shared_state.lock().unwrap();
                            state.config.start_hour = sh;
                            state.config.start_minute = sm;
                            state.config.end_hour = eh;
                            state.config.end_minute = em;
                            state.config.auto_start = auto_start;

                            let _ = state.config.save();
                            let _ = state.config.sync_autostart_registry();
                            state.evaluate_and_sync();
                        }

                        let title: Vec<u16> = "Settings Saved\0".encode_utf16().collect();
                        let msg: Vec<u16> = "Settings saved successfully.\0".encode_utf16().collect();
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
        let title_wide: Vec<u16> = "Kid Internet Lock - Admin Settings\0".encode_utf16().collect();
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

        let width = 530;
        let height = 590;
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
        // Group 1: Block schedule
        // -------------------------------------------------------------
        let grp1_title: Vec<u16> = " 🕒 Block Schedule \0".encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp1_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            12,
            478,
            98,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let lbl_start: Vec<u16> = "Block start:\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_start.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 40, 70, 20, hwnd, null_mut(), hinstance, null_mut());

        let edit_start_h = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 110, 38, 40, 24, hwnd, ID_EDIT_START_H as _, hinstance, null_mut());
        let colon1: Vec<u16> = ":\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), colon1.as_ptr(), WS_CHILD | WS_VISIBLE, 155, 40, 10, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_start_m = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 168, 38, 40, 24, hwnd, ID_EDIT_START_M as _, hinstance, null_mut());

        let lbl_end: Vec<u16> = "Block end:\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_end.as_ptr(), WS_CHILD | WS_VISIBLE, 260, 40, 70, 20, hwnd, null_mut(), hinstance, null_mut());

        let edit_end_h = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 335, 38, 40, 24, hwnd, ID_EDIT_END_H as _, hinstance, null_mut());
        let colon2: Vec<u16> = ":\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), colon2.as_ptr(), WS_CHILD | WS_VISIBLE, 380, 40, 10, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_end_m = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_CENTER as u32) | (ES_NUMBER as u32), 393, 38, 40, 24, hwnd, ID_EDIT_END_M as _, hinstance, null_mut());

        let note_text: Vec<u16> = "※ Default 00:00 - 07:00 / Time ranges crossing midnight (e.g. 23:30 - 06:30) are also supported\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), note_text.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 74, 445, 20, hwnd, null_mut(), hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 2: Temporary allow & manual control
        // -------------------------------------------------------------
        let grp2_title: Vec<u16> = " ⚡ Temporary Allow & Manual Control \0".encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp2_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            120,
            478,
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
            145,
            445,
            24,
            hwnd,
            ID_LABEL_STATUS as _,
            hinstance,
            null_mut(),
        );

        let btn_30_txt: Vec<u16> = "Allow 30 Minutes\0".encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_30_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 36, 180, 100, 32, hwnd, ID_BTN_TEMP_30 as _, hinstance, null_mut());

        let btn_60_txt: Vec<u16> = "Allow 1 Hour\0".encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_60_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 142, 180, 100, 32, hwnd, ID_BTN_TEMP_60 as _, hinstance, null_mut());

        let btn_cancel_txt: Vec<u16> = "Cancel Temporary Allow\0".encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_cancel_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 248, 180, 100, 32, hwnd, ID_BTN_TEMP_CANCEL as _, hinstance, null_mut());

        let btn_toggle_txt: Vec<u16> = "Block / Unblock Now\0".encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_toggle_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 354, 180, 126, 32, hwnd, ID_BTN_TOGGLE_NOW as _, hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 3: Change admin password
        // -------------------------------------------------------------
        let grp3_title: Vec<u16> = " 🔑 Change Admin Password \0".encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp3_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            245,
            478,
            160,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let lbl_old: Vec<u16> = "Current password:\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_old.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 275, 110, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_old_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 150, 273, 170, 24, hwnd, ID_EDIT_OLD_PWD as _, hinstance, null_mut());

        let lbl_new: Vec<u16> = "New password:\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_new.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 308, 110, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_new_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 150, 306, 170, 24, hwnd, ID_EDIT_NEW_PWD as _, hinstance, null_mut());

        let lbl_conf: Vec<u16> = "Confirm new password:\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), lbl_conf.as_ptr(), WS_CHILD | WS_VISIBLE, 36, 341, 110, 20, hwnd, null_mut(), hinstance, null_mut());
        let edit_confirm_pwd = CreateWindowExW(WS_EX_CLIENTEDGE, edit_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32), 150, 339, 170, 24, hwnd, ID_EDIT_CONFIRM_PWD as _, hinstance, null_mut());

        disable_ime(edit_old_pwd);
        disable_ime(edit_new_pwd);
        disable_ime(edit_confirm_pwd);

        let btn_pwd_txt: Vec<u16> = "Change Password\0".encode_utf16().collect();
        CreateWindowExW(0, btn_class.as_ptr(), btn_pwd_txt.as_ptr(), WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32), 340, 290, 135, 42, hwnd, ID_BTN_CHANGE_PWD as _, hinstance, null_mut());

        let label_pwd_msg = CreateWindowExW(0, static_class.as_ptr(), [0u16].as_ptr(), WS_CHILD | WS_VISIBLE, 36, 374, 440, 20, hwnd, ID_LABEL_PWD_MSG as _, hinstance, null_mut());

        // -------------------------------------------------------------
        // Group 4: Startup settings
        // -------------------------------------------------------------
        let grp4_title: Vec<u16> = " ⚙️ Startup \0".encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp4_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            18,
            415,
            478,
            60,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let chk_txt: Vec<u16> = "Run automatically in the background at Windows logon\0".encode_utf16().collect();
        let chk_autostart = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            chk_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_AUTOCHECKBOX as u32),
            36,
            438,
            380,
            24,
            hwnd,
            ID_CHK_AUTOSTART as _,
            hinstance,
            null_mut(),
        );

        // -------------------------------------------------------------
        // Bottom Action Buttons
        // -------------------------------------------------------------
        let save_txt: Vec<u16> = "Save Settings\0".encode_utf16().collect();
        let btn_save = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            save_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON as u32),
            275,
            495,
            110,
            36,
            hwnd,
            ID_BTN_SAVE as _,
            hinstance,
            null_mut(),
        );

        let close_txt: Vec<u16> = "Close\0".encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            close_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32),
            395,
            495,
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

        // Populate initial values from config
        {
            let state = shared_state.lock().unwrap();
            let sh_str: Vec<u16> = format!("{:02}\0", state.config.start_hour).encode_utf16().collect();
            let sm_str: Vec<u16> = format!("{:02}\0", state.config.start_minute).encode_utf16().collect();
            let eh_str: Vec<u16> = format!("{:02}\0", state.config.end_hour).encode_utf16().collect();
            let em_str: Vec<u16> = format!("{:02}\0", state.config.end_minute).encode_utf16().collect();

            SetWindowTextW(edit_start_h, sh_str.as_ptr());
            SetWindowTextW(edit_start_m, sm_str.as_ptr());
            SetWindowTextW(edit_end_h, eh_str.as_ptr());
            SetWindowTextW(edit_end_m, em_str.as_ptr());

            SendMessageW(
                chk_autostart,
                BM_SETCHECK,
                if state.config.auto_start { 1 } else { 0 },
                0,
            );
        }

        let ctx = Box::into_raw(Box::new(SettingsContext {
            shared_state: shared_state.clone(),
            edit_start_h,
            edit_start_m,
            edit_end_h,
            edit_end_m,
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
