#![windows_subsystem = "windows"]

mod admin;
mod config;
mod firewall;
mod icon;
mod scheduler;
mod service;
mod single_instance;
mod tray;
mod ui;
mod watchdog;

use config::AppConfig;
use scheduler::AppState;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let silent = args.iter().any(|a| a == "--silent");

    // 0a. Service mode: Windows service that revives the app when it is killed
    if args.iter().any(|a| a == "--service") {
        service::run();
        return;
    }

    // 0b. Hidden watchdog helper mode: relaunch the main app if it disappears.
    if let Some(pos) = args.iter().position(|a| a == "--watchdog") {
        let pid = args
            .get(pos + 1)
            .and_then(|p| p.parse::<u32>().ok())
            .unwrap_or(0);
        if pid != 0 && admin::is_elevated() {
            std::process::exit(watchdog::run_watchdog(pid));
        }
        return;
    }

    // 1. Verify administrative privileges required for Windows Firewall control
    if !admin::is_elevated() {
        if silent {
            // Auto-restart probe: stay quiet unless the app is really gone.
            if single_instance::is_running(single_instance::INSTANCE_MUTEX_NAME) {
                return;
            }
            admin::relaunch_as_admin();
            return;
        }
        if !admin::relaunch_as_admin() {
            unsafe {
                let title: Vec<u16> = "Administrator Rights Required\0".encode_utf16().collect();
                let msg: Vec<u16> = "Kid Internet Lock needs administrator rights to control Windows Firewall.\nPlease choose 'Run as administrator'.\0"
                    .encode_utf16()
                    .collect();
                MessageBoxW(null_mut(), msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING);
            }
        }
        return;
    }

    // 2. Prevent duplicate execution using system-wide Mutex
    let _instance_guard = match single_instance::acquire_single_instance(single_instance::INSTANCE_MUTEX_NAME) {
        Some(guard) => guard,
        None => {
            if !silent {
                unsafe {
                    let title: Vec<u16> = "Notice\0".encode_utf16().collect();
                    let msg: Vec<u16> = "Kid Internet Lock is already running.\nPlease check the tray icon in the bottom-right corner of the taskbar.\0"
                        .encode_utf16()
                        .collect();
                    MessageBoxW(null_mut(), msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
                }
            }
            return;
        }
    };

    // 3. Keep running even if terminated from Task Manager:
    //    a privileged service revives the app instantly, and an in-session
    //    watchdog takes over if the service is unavailable.
    service::ensure_installed();
    watchdog::start_guardian();

    // 4. Load user configuration
    let config = AppConfig::load();
    if config.auto_start {
        let _ = config.sync_autostart_registry();
    }

    // 5. Initialize shared state
    let shared_state = Arc::new(Mutex::new(AppState::new(config)));

    // 6. Run system tray loop
    if let Err(err) = tray::run_tray_app(shared_state) {
        unsafe {
            let title: Vec<u16> = "Error\0".encode_utf16().collect();
            let msg: Vec<u16> = format!("An error occurred while running the application:\n{}\0", err)
                .encode_utf16()
                .collect();
            MessageBoxW(null_mut(), msg.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING);
        }
    }

    // 7. Intentional exit: stop the guardian so it does not resurrect the app
    watchdog::stop_guardian();
}
