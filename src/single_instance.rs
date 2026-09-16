use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::{CreateMutexW, OpenMutexW, MUTEX_ALL_ACCESS};

pub const INSTANCE_MUTEX_NAME: &str = "Global\\KidInternetLock_SingleInstance_Mutex";

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

/// Attempts to acquire a single-instance lock across the system.
/// Returns Some(guard) if this is the only instance, or None if already running.
pub fn acquire_single_instance(name: &str) -> Option<SingleInstanceGuard> {
    let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateMutexW(null_mut(), 1, name_wide.as_ptr());
        if handle.is_null() {
            return None;
        }

        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            return None;
        }

        Some(SingleInstanceGuard { handle })
    }
}

/// Checks whether another process is currently holding the named lock.
pub fn is_running(name: &str) -> bool {
    let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = OpenMutexW(MUTEX_ALL_ACCESS, 0, name_wide.as_ptr());
        if handle.is_null() {
            return false;
        }
        CloseHandle(handle);
        true
    }
}
