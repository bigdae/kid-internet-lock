use crate::firewall::FirewallManager;
use crate::icon::DynamicIcons;
use crate::scheduler::{SharedAppState, StatusDetail};
use crate::ui::password_dialog::prompt_admin_password;
use crate::ui::settings_dialog::open_settings_dialog;
use crate::watchdog;
use std::mem::zeroed;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW,
    KillTimer, PostQuitMessage, RegisterClassExW, SetForegroundWindow, SetTimer,
    SetWindowLongPtrW, TrackPopupMenuEx, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    MF_DISABLED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, TPM_RIGHTBUTTON,
    WM_APP, WM_COMMAND, WM_CLOSE, WM_DESTROY, WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
    WM_TIMER, WNDCLASSEXW,
};

const WM_TRAYICON: u32 = WM_APP + 10;
const ID_SYNC_TIMER: usize = 999;

const CMD_TEMP_30: usize = 3001;
const CMD_TEMP_60: usize = 3002;
const CMD_TEMP_CANCEL: usize = 3003;
const CMD_TOGGLE_NOW: usize = 3004;
const CMD_SETTINGS: usize = 3005;
const CMD_EXIT: usize = 3006;

struct TrayContext {
    shared_state: SharedAppState,
    icons: DynamicIcons,
    nid: NOTIFYICONDATAW,
    last_status: Option<StatusDetail>,
}

unsafe fn update_tray(ctx: &mut TrayContext) {
    unsafe {
        let status = {
            let mut state = ctx.shared_state.lock().unwrap();
            state.evaluate_and_sync()
        };

        let current_icon = if status.is_blocked {
            ctx.icons.red_icon
        } else {
            ctx.icons.green_icon
        };

        ctx.nid.hIcon = current_icon;

        // Copy up to 127 characters for tooltip
        let mut tip_chars = [0u16; 128];
        let tip_src = status.tooltip.encode_utf16().take(127);
        for (i, c) in tip_src.enumerate() {
            tip_chars[i] = c;
        }
        ctx.nid.szTip = tip_chars;

        Shell_NotifyIconW(NIM_MODIFY, &ctx.nid);
        ctx.last_status = Some(status);
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ctx_ptr = GetWindowLongPtrW(hwnd, 0) as *mut TrayContext;

        match msg {
            WM_TIMER => {
                if wparam == ID_SYNC_TIMER && !ctx_ptr.is_null() {
                    let ctx = &mut *ctx_ptr;
                    update_tray(ctx);
                }
                0
            }
            WM_TRAYICON => {
                let event = (lparam & 0xFFFF) as u32;
                if ctx_ptr.is_null() {
                    return 0;
                }
                let ctx = &mut *ctx_ptr;

                if event == WM_RBUTTONUP {
                    let mut pt: POINT = zeroed();
                    GetCursorPos(&mut pt);

                    let hmenu = CreatePopupMenu();
                    if !hmenu.is_null() {
                        let current_status = ctx.last_status.as_ref();
                        let is_blocked = current_status.map(|s| s.is_blocked).unwrap_or(false);
                        let has_temp_allow = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.temp_allow_until.is_some()
                        };

                        let status_label = if is_blocked {
                            "Kid Internet Lock [🔴 Internet Blocked]\0"
                        } else {
                            "Kid Internet Lock [🟢 Internet Allowed]\0"
                        };
                        let status_wide: Vec<u16> = status_label.encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING | MF_GRAYED | MF_DISABLED, 0, status_wide.as_ptr());
                        AppendMenuW(hmenu, MF_SEPARATOR, 0, null_mut());

                        let t30: Vec<u16> = "Allow 30 Minutes\0".encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING, CMD_TEMP_30, t30.as_ptr());

                        let t60: Vec<u16> = "Allow 1 Hour\0".encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING, CMD_TEMP_60, t60.as_ptr());

                        if has_temp_allow {
                            let tcancel: Vec<u16> = "Cancel Temporary Allow\0".encode_utf16().collect();
                            AppendMenuW(hmenu, MF_STRING, CMD_TEMP_CANCEL, tcancel.as_ptr());
                        }

                        AppendMenuW(hmenu, MF_SEPARATOR, 0, null_mut());

                        let toggle_txt: Vec<u16> = "Block / Unblock Now\0".encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING, CMD_TOGGLE_NOW, toggle_txt.as_ptr());

                        let settings_txt: Vec<u16> = "Admin Settings (S)...\0".encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING, CMD_SETTINGS, settings_txt.as_ptr());

                        AppendMenuW(hmenu, MF_SEPARATOR, 0, null_mut());

                        let exit_txt: Vec<u16> = "Exit (X)\0".encode_utf16().collect();
                        AppendMenuW(hmenu, MF_STRING, CMD_EXIT, exit_txt.as_ptr());

                        SetForegroundWindow(hwnd);
                        TrackPopupMenuEx(hmenu, TPM_RIGHTBUTTON, pt.x, pt.y, hwnd, null_mut());
                        DestroyMenu(hmenu);
                    }
                } else if event == WM_LBUTTONDBLCLK {
                    let config = {
                        let state = ctx.shared_state.lock().unwrap();
                        state.config.clone()
                    };
                    if prompt_admin_password(null_mut(), &config, "Kid Internet Lock - Admin Authentication") {
                        open_settings_dialog(ctx.shared_state.clone());
                    }
                }
                0
            }
            WM_COMMAND => {
                let cmd_id = (wparam & 0xFFFF) as usize;
                if ctx_ptr.is_null() {
                    return 0;
                }
                let ctx = &mut *ctx_ptr;

                match cmd_id {
                    CMD_TEMP_30 => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        if prompt_admin_password(null_mut(), &config, "Allow 30 Minutes - Admin Authentication") {
                            {
                                let mut state = ctx.shared_state.lock().unwrap();
                                state.set_temporary_allow(30);
                                state.evaluate_and_sync();
                            }
                            update_tray(ctx);
                        }
                    }
                    CMD_TEMP_60 => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        if prompt_admin_password(null_mut(), &config, "Allow 1 Hour - Admin Authentication") {
                            {
                                let mut state = ctx.shared_state.lock().unwrap();
                                state.set_temporary_allow(60);
                                state.evaluate_and_sync();
                            }
                            update_tray(ctx);
                        }
                    }
                    CMD_TEMP_CANCEL => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        if prompt_admin_password(null_mut(), &config, "Cancel Temporary Allow - Admin Authentication") {
                            {
                                let mut state = ctx.shared_state.lock().unwrap();
                                state.cancel_temporary_allow();
                                state.clear_manual_override();
                                state.evaluate_and_sync();
                            }
                            update_tray(ctx);
                        }
                    }
                    CMD_TOGGLE_NOW => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        if prompt_admin_password(null_mut(), &config, "Block / Unblock Now - Admin Authentication") {
                            {
                                let mut state = ctx.shared_state.lock().unwrap();
                                state.toggle_manual_override();
                                state.evaluate_and_sync();
                            }
                            update_tray(ctx);
                        }
                    }
                    CMD_SETTINGS => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        if prompt_admin_password(null_mut(), &config, "Kid Internet Lock - Settings Authentication") {
                            open_settings_dialog(ctx.shared_state.clone());
                        }
                    }
                    CMD_EXIT => {
                        let config = {
                            let state = ctx.shared_state.lock().unwrap();
                            state.config.clone()
                        };
                        // Per requirements: must authenticate to exit so children cannot close it
                        if prompt_admin_password(null_mut(), &config, "Kid Internet Lock - Exit Authentication") {
                            // Intentional exit: stop watchdog tasks and auto-restart guard
                            watchdog::stop_guardian();
                            // Safely restore firewall before normal exit
                            let _ = FirewallManager::unblock_internet();
                            Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
                            DestroyWindow(hwnd);
                        }
                    }
                    _ => {}
                }
                0
            }
            WM_ENDSESSION => {
                // Windows is logging off or shutting down: do not fight it,
                // but keep the guard task so protection resumes after logon.
                if wparam != 0 {
                    watchdog::signal_shutdown();
                }
                0
            }
            WM_CLOSE => {
                // Refuse external close requests (e.g. Task Manager "End task"):
                // the only way to exit is the password-protected tray menu.
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, ID_SYNC_TIMER);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Runs the system tray application and Windows message pump.
pub fn run_tray_app(shared_state: SharedAppState) -> Result<(), String> {
    let class_name: Vec<u16> = "KidInternetLock_TrayWindow\0".encode_utf16().collect();
    let icons = DynamicIcons::new()?;

    unsafe {
        let hinstance = GetModuleHandleW(null_mut());

        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(tray_wnd_proc);
        wc.hInstance = hinstance;
        wc.cbWndExtra = std::mem::size_of::<*mut ()>() as i32;
        wc.lpszClassName = class_name.as_ptr();

        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create tray message window".to_string());
        }

        let mut nid: NOTIFYICONDATAW = zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = icons.green_icon;

        let default_tip: Vec<u16> = "Kid Internet Lock\0".encode_utf16().collect();
        for (i, &c) in default_tip.iter().enumerate().take(127) {
            nid.szTip[i] = c;
        }

        if Shell_NotifyIconW(NIM_ADD, &nid) == 0 {
            return Err("Failed to add system tray icon".to_string());
        }

        let mut ctx = Box::new(TrayContext {
            shared_state: shared_state.clone(),
            icons,
            nid,
            last_status: None,
        });

        SetWindowLongPtrW(hwnd, 0, &mut *ctx as *mut _ as isize);

        // Perform initial evaluation & icon/tooltip sync
        update_tray(&mut *ctx);

        // Timer to evaluate schedule every 5 seconds (5000ms)
        SetTimer(hwnd, ID_SYNC_TIMER, 5000, None);

        // Win32 message loop
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Clean up tray icon on exit
        Shell_NotifyIconW(NIM_DELETE, &ctx.nid);

        Ok(())
    }
}
