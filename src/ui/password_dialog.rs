use crate::config::AppConfig;
use crate::ui::font::{create_ui_font, set_control_fonts};
use crate::ui::ime::disable_ime;
use std::mem::zeroed;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    DeleteObject, GetSysColorBrush, SetBkMode, SetTextColor,
    COLOR_BTNFACE, HBRUSH, HDC, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SetActiveWindow, SetFocus, VK_ESCAPE, VK_RETURN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowTextW, IsWindow, PostMessageW,
    RegisterClassExW, SendMessageW, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowTextW, ShowWindow, TranslateMessage, BS_DEFPUSHBUTTON,
    BS_PUSHBUTTON, CS_HREDRAW, CS_VREDRAW, EN_SETFOCUS, ES_AUTOHSCROLL, ES_PASSWORD, GWLP_USERDATA,
    IDC_ARROW, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_DESTROY, WM_KEYDOWN, WM_SETFOCUS,
    WNDCLASSEXW, WS_CAPTION, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE,
};

const ID_EDIT_PWD: usize = 101;
const ID_BTN_OK: usize = 102;
const ID_BTN_CANCEL: usize = 103;
const ID_LABEL_ERROR: usize = 104;

struct DialogContext {
    config: AppConfig,
    verified: bool,
    edit_hwnd: HWND,
    error_hwnd: HWND,
}

unsafe extern "system" fn pwd_dlg_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogContext;

        match msg {
            WM_SETFOCUS => {
                if !ctx_ptr.is_null() {
                    SetFocus((*ctx_ptr).edit_hwnd);
                }
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as usize;
                let notify = ((wparam >> 16) & 0xFFFF) as u32;
                if notify == EN_SETFOCUS && !ctx_ptr.is_null() {
                    disable_ime((*ctx_ptr).edit_hwnd);
                    0
                } else if id == ID_BTN_OK && !ctx_ptr.is_null() {
                    let ctx = &mut *ctx_ptr;
                    let len = GetWindowTextLengthW(ctx.edit_hwnd) as usize;
                    let mut buf = vec![0u16; len + 1];
                    GetWindowTextW(ctx.edit_hwnd, buf.as_mut_ptr(), (len + 1) as i32);
                    buf.pop(); // remove null terminator
                    let input = String::from_utf16_lossy(&buf);

                    if ctx.config.verify_password(&input) {
                        ctx.verified = true;
                        DestroyWindow(hwnd);
                    } else {
                        let err_text = "비밀번호가 올바르지 않습니다.\0".encode_utf16().collect::<Vec<u16>>();
                        SetWindowTextW(ctx.error_hwnd, err_text.as_ptr());
                        SetWindowTextW(ctx.edit_hwnd, [0u16].as_ptr());
                        SetFocus(ctx.edit_hwnd);
                    }
                    0
                } else if id == ID_BTN_CANCEL {
                    DestroyWindow(hwnd);
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_CTLCOLORSTATIC => {
                let control_hwnd = lparam as HWND;
                if !ctx_ptr.is_null() && control_hwnd == (*ctx_ptr).error_hwnd {
                    let hdc = wparam as HDC;
                    SetTextColor(hdc, 0x000000C8); // Clean red text (BGR)
                    SetBkMode(hdc, TRANSPARENT as i32);
                    return GetSysColorBrush(COLOR_BTNFACE) as LRESULT;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostMessageW(hwnd, 0, 0, 0); // Wake up GetMessageW immediately
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Prompts the user for the administrator password via a modal dialog.
/// Returns true if authentication succeeded, false otherwise.
pub fn prompt_admin_password(parent_hwnd: HWND, config: &AppConfig, title: &str) -> bool {
    let class_name: Vec<u16> = "KidInternetLock_PwdDlg\0".encode_utf16().collect();
    let title_wide: Vec<u16> = format!("{}\0", title).encode_utf16().collect();

    unsafe {
        let hinstance = GetModuleHandleW(null_mut());

        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(pwd_dlg_proc);
        wc.hInstance = hinstance;
        wc.hCursor = windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW(
            null_mut(),
            IDC_ARROW,
        );
        wc.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);

        let width = 360;
        let height = 210;
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - width) / 2;
        let y = (screen_h - height) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_TOPMOST,
            class_name.as_ptr(),
            title_wide.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            width,
            height,
            parent_hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return false;
        }

        disable_ime(hwnd);

        let font = create_ui_font(13, false);
        let font_bold = create_ui_font(13, true);

        // Instruction label
        let label_text: Vec<u16> = "관리자 비밀번호를 입력해주세요:\0".encode_utf16().collect();
        let static_class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let edit_class: Vec<u16> = "EDIT\0".encode_utf16().collect();
        let btn_class: Vec<u16> = "BUTTON\0".encode_utf16().collect();

        CreateWindowExW(
            0,
            static_class.as_ptr(),
            label_text.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            20,
            300,
            22,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        // Native Password edit input with WS_EX_CLIENTEDGE (sunken 3D border)
        let edit_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (ES_PASSWORD as u32) | (ES_AUTOHSCROLL as u32),
            24,
            50,
            296,
            26,
            hwnd,
            ID_EDIT_PWD as _,
            hinstance,
            null_mut(),
        );

        disable_ime(edit_hwnd);

        // Error message label
        let error_hwnd = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            82,
            296,
            20,
            hwnd,
            ID_LABEL_ERROR as _,
            hinstance,
            null_mut(),
        );

        // OK Button
        let ok_text: Vec<u16> = "확인\0".encode_utf16().collect();
        let btn_ok = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            ok_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON as u32),
            140,
            118,
            86,
            32,
            hwnd,
            ID_BTN_OK as _,
            hinstance,
            null_mut(),
        );

        // Cancel Button
        let cancel_text: Vec<u16> = "취소\0".encode_utf16().collect();
        let _btn_cancel = CreateWindowExW(
            0,
            btn_class.as_ptr(),
            cancel_text.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32),
            234,
            118,
            86,
            32,
            hwnd,
            ID_BTN_CANCEL as _,
            hinstance,
            null_mut(),
        );

        set_control_fonts(hwnd, font);
        SendMessageW(btn_ok, windows_sys::Win32::UI::WindowsAndMessaging::WM_SETFONT, font_bold as _, 1);

        let mut ctx = DialogContext {
            config: config.clone(),
            verified: false,
            edit_hwnd,
            error_hwnd,
        };

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut ctx as *mut _ as isize);

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(edit_hwnd);

        // Modal message loop:
        // Intercept VK_RETURN and VK_ESCAPE globally without messing with edit control internals.
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_KEYDOWN {
                if msg.wParam == VK_RETURN as usize {
                    PostMessageW(hwnd, WM_COMMAND, ID_BTN_OK, 0);
                    continue;
                } else if msg.wParam == VK_ESCAPE as usize {
                    PostMessageW(hwnd, WM_COMMAND, ID_BTN_CANCEL, 0);
                    continue;
                }
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if IsWindow(hwnd) == 0 {
                break;
            }
        }

        DeleteObject(font);
        DeleteObject(font_bold);

        ctx.verified
    }
}
