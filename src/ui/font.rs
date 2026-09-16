use std::mem::zeroed;
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, HFONT, LOGFONTW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, SendMessageW, WM_SETFONT,
};

/// Creates a crisp modern UI font (Malgun Gothic or Segoe UI) with specified point size.
pub fn create_ui_font(size_pixels: i32, is_bold: bool) -> HFONT {
    unsafe {
        let mut lf: LOGFONTW = zeroed();
        lf.lfHeight = -size_pixels;
        lf.lfWeight = if is_bold { 700 } else { 400 };
        lf.lfCharSet = 1; // DEFAULT_CHARSET

        // "Malgun Gothic" for clean rendering of UI text
        let font_name = "Malgun Gothic";
        for (i, c) in font_name.encode_utf16().enumerate().take(31) {
            lf.lfFaceName[i] = c;
        }

        CreateFontIndirectW(&lf)
    }
}

unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    unsafe {
        SendMessageW(hwnd, WM_SETFONT, lparam as _, 1);
    }
    1 // TRUE
}

/// Applies the given font to the window and all its child controls.
pub fn set_control_fonts(parent_hwnd: HWND, font: HFONT) {
    unsafe {
        SendMessageW(parent_hwnd, WM_SETFONT, font as _, 1);
        EnumChildWindows(parent_hwnd, Some(enum_child_proc), font as _);
    }
}
