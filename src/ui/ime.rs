use std::ptr::null_mut;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::Ime::ImmAssociateContext;

/// Detaches the IME from a control so keystrokes are always treated as
/// English letters, even while the Korean IME is active elsewhere.
pub fn disable_ime(hwnd: HWND) {
    unsafe {
        ImmAssociateContext(hwnd, null_mut());
    }
}
