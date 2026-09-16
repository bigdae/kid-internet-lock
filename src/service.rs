use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::Security::{
    DuplicateTokenEx, GetTokenInformation, SecurityImpersonation, TokenLinkedToken, TokenPrimary,
    TOKEN_ALL_ACCESS, TOKEN_LINKED_TOKEN,
};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows_sys::Win32::System::Services::{
    ChangeServiceConfig2W, CloseServiceHandle, ControlService, CreateServiceW, DeleteService,
    OpenSCManagerW, OpenServiceW, QueryServiceStatus, RegisterServiceCtrlHandlerW,
    SetServiceStatus, StartServiceCtrlDispatcherW, StartServiceW, SC_MANAGER_CONNECT,
    SC_MANAGER_CREATE_SERVICE, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_ALL_ACCESS,
    SERVICE_AUTO_START, SERVICE_CONFIG_DESCRIPTION, SERVICE_CONTROL_INTERROGATE,
    SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_DESCRIPTIONW, SERVICE_ERROR_IGNORE,
    SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOP_PENDING, SERVICE_STOPPED,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateProcessAsUserW, OpenMutexW, SetEvent, WaitForSingleObject,
    CREATE_UNICODE_ENVIRONMENT, MUTEX_ALL_ACCESS, PROCESS_INFORMATION, STARTUPINFOW,
};

use crate::single_instance::INSTANCE_MUTEX_NAME;

const SERVICE_NAME: &str = "KidInternetLockGuard";
const SERVICE_DISPLAY_NAME: &str = "야간 인터넷 지킴이 감시 서비스";
const SERVICE_DESCRIPTION: &str =
    "야간 인터넷 지킴이가 종료되면 즉시 다시 실행합니다. (Kid Internet Lock Guard)";

static STATUS_HANDLE: AtomicIsize = AtomicIsize::new(0);
static STOP_EVENT: AtomicIsize = AtomicIsize::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn exe_path_string() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

fn is_app_running() -> bool {
    unsafe {
        let name = wide(INSTANCE_MUTEX_NAME);
        let handle = OpenMutexW(MUTEX_ALL_ACCESS, 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        CloseHandle(handle);
        true
    }
}

/// Registers the watchdog service if needed and makes sure it is running.
pub fn ensure_installed() {
    unsafe {
        let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CREATE_SERVICE);
        if scm.is_null() {
            return;
        }

        let name = wide(SERVICE_NAME);
        let existing = OpenServiceW(scm, name.as_ptr(), SERVICE_ALL_ACCESS);
        if !existing.is_null() {
            let mut status: SERVICE_STATUS = zeroed();
            if QueryServiceStatus(existing, &mut status) != 0
                && status.dwCurrentState == SERVICE_STOPPED
            {
                StartServiceW(existing, 0, null());
            }
            CloseServiceHandle(existing);
            CloseServiceHandle(scm);
            return;
        }

        if let Some(exe) = exe_path_string() {
            let binary = wide(&format!("\"{}\" --service", exe));
            let display = wide(SERVICE_DISPLAY_NAME);

            let service = CreateServiceW(
                scm,
                name.as_ptr(),
                display.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_IGNORE,
                binary.as_ptr(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
            );

            if !service.is_null() {
                let description = wide(SERVICE_DESCRIPTION);
                let mut description_struct = SERVICE_DESCRIPTIONW {
                    lpDescription: description.as_ptr() as *mut u16,
                };
                ChangeServiceConfig2W(
                    service,
                    SERVICE_CONFIG_DESCRIPTION,
                    &mut description_struct as *mut _ as *const core::ffi::c_void,
                );
                StartServiceW(service, 0, null());
                CloseServiceHandle(service);
            }
        }

        CloseServiceHandle(scm);
    }
}

/// Stops and removes the watchdog service (used on an intentional exit).
pub fn stop_and_remove() {
    unsafe {
        let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return;
        }

        let name = wide(SERVICE_NAME);
        let service = OpenServiceW(scm, name.as_ptr(), SERVICE_ALL_ACCESS);
        if !service.is_null() {
            let mut status: SERVICE_STATUS = zeroed();
            ControlService(service, SERVICE_CONTROL_STOP, &mut status);

            for _ in 0..50 {
                if QueryServiceStatus(service, &mut status) == 0
                    || status.dwCurrentState == SERVICE_STOPPED
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }

            DeleteService(service);
            CloseServiceHandle(service);
        }

        CloseServiceHandle(scm);
    }
}

unsafe extern "system" fn ctrl_handler(control: u32) {
    unsafe {
        let handle = STATUS_HANDLE.load(Ordering::SeqCst);
        if handle == 0 {
            return;
        }
        let status_handle = handle as SERVICE_STATUS_HANDLE;

        match control {
            SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
                let event = STOP_EVENT.load(Ordering::SeqCst);
                if event != 0 {
                    SetEvent(event as HANDLE);
                }
                let status = SERVICE_STATUS {
                    dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                    dwCurrentState: SERVICE_STOP_PENDING,
                    dwControlsAccepted: 0,
                    dwWin32ExitCode: 0,
                    dwServiceSpecificExitCode: 0,
                    dwCheckPoint: 1,
                    dwWaitHint: 5000,
                };
                SetServiceStatus(status_handle, &status);
            }
            SERVICE_CONTROL_INTERROGATE => {
                let status = SERVICE_STATUS {
                    dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                    dwCurrentState: SERVICE_RUNNING,
                    dwControlsAccepted: SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
                    dwWin32ExitCode: 0,
                    dwServiceSpecificExitCode: 0,
                    dwCheckPoint: 0,
                    dwWaitHint: 0,
                };
                SetServiceStatus(status_handle, &status);
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    unsafe {
        let name = wide(SERVICE_NAME);
        let handle = RegisterServiceCtrlHandlerW(name.as_ptr(), Some(ctrl_handler));
        if handle.is_null() {
            return;
        }
        STATUS_HANDLE.store(handle as isize, Ordering::SeqCst);

        let stop = CreateEventW(null_mut(), 1, 0, null_mut());
        if stop.is_null() {
            return;
        }
        STOP_EVENT.store(stop as isize, Ordering::SeqCst);

        let mut status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: SERVICE_RUNNING,
            dwControlsAccepted: SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
            dwWin32ExitCode: 0,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: 0,
        };
        SetServiceStatus(handle, &status);

        let mut last_launch: Option<Instant> = None;
        loop {
            if WaitForSingleObject(stop, 500) == WAIT_OBJECT_0 {
                break;
            }

            if is_app_running() {
                last_launch = None;
                continue;
            }

            if let Some(launched) = last_launch {
                if launched.elapsed() < Duration::from_secs(5) {
                    continue;
                }
            }

            launch_in_user_session();
            last_launch = Some(Instant::now());
        }

        status.dwCurrentState = SERVICE_STOPPED;
        SetServiceStatus(handle, &status);
        CloseHandle(stop);
    }
}

/// Entry point when the executable is started by the Service Control Manager.
pub fn run() {
    unsafe {
        let name = wide(SERVICE_NAME);
        let table = [
            SERVICE_TABLE_ENTRYW {
                lpServiceName: name.as_ptr() as *mut u16,
                lpServiceProc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                lpServiceName: null_mut(),
                lpServiceProc: None,
            },
        ];
        StartServiceCtrlDispatcherW(table.as_ptr());
    }
}

unsafe fn launch_in_user_session() {
    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();
        if session_id == u32::MAX {
            return;
        }

        let mut session_token: HANDLE = null_mut();
        if WTSQueryUserToken(session_id, &mut session_token) == 0 {
            return;
        }

        let mut linked: TOKEN_LINKED_TOKEN = zeroed();
        let mut returned = 0u32;
        let has_linked = GetTokenInformation(
            session_token,
            TokenLinkedToken,
            &mut linked as *mut _ as *mut core::ffi::c_void,
            size_of::<TOKEN_LINKED_TOKEN>() as u32,
            &mut returned,
        ) != 0
            && !linked.LinkedToken.is_null();

        let source_token = if has_linked {
            linked.LinkedToken
        } else {
            session_token
        };

        let mut primary: HANDLE = null_mut();
        if DuplicateTokenEx(
            source_token,
            TOKEN_ALL_ACCESS,
            null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        ) == 0
        {
            if has_linked {
                CloseHandle(linked.LinkedToken);
            }
            CloseHandle(session_token);
            return;
        }

        let mut env: *mut core::ffi::c_void = null_mut();
        let env_ok = CreateEnvironmentBlock(&mut env, primary, 0) != 0;

        if let Some(exe) = exe_path_string() {
            let mut command = wide(&format!("\"{}\" --silent", exe));
            let mut desktop = wide("winsta0\\default");

            let mut si: STARTUPINFOW = zeroed();
            si.cb = size_of::<STARTUPINFOW>() as u32;
            si.lpDesktop = desktop.as_mut_ptr();

            let mut pi: PROCESS_INFORMATION = zeroed();
            CreateProcessAsUserW(
                primary,
                null(),
                command.as_mut_ptr(),
                null(),
                null(),
                0,
                CREATE_UNICODE_ENVIRONMENT,
                if env_ok { env as *const _ } else { null() },
                null(),
                &si,
                &mut pi,
            );

            if !pi.hProcess.is_null() {
                CloseHandle(pi.hProcess);
            }
            if !pi.hThread.is_null() {
                CloseHandle(pi.hThread);
            }
        }

        if env_ok {
            DestroyEnvironmentBlock(env);
        }
        CloseHandle(primary);
        if has_linked {
            CloseHandle(linked.LinkedToken);
        }
        CloseHandle(session_token);
    }
}
