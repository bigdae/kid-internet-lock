use std::mem::size_of;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Checks whether the current process is running with elevated administrator privileges.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION {
            TokenIsElevated: 0,
        };
        let mut return_length = 0u32;
        let success = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        CloseHandle(token);
        success != 0 && elevation.TokenIsElevated != 0
    }
}

/// Relaunches the current executable with administrative privileges (UAC prompt).
pub fn relaunch_as_admin() -> bool {
    let Ok(exe_path) = std::env::current_exe() else {
        return false;
    };

    let exe_str = exe_path.to_string_lossy().to_string();
    let exe_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
    let verb_wide: Vec<u16> = "runas\0".encode_utf16().collect();

    unsafe {
        let res = ShellExecuteW(
            null_mut(),
            verb_wide.as_ptr(),
            exe_wide.as_ptr(),
            null_mut(),
            null_mut(),
            SW_SHOWNORMAL,
        );
        (res as usize) > 32
    }
}
