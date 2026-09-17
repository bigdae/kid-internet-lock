//! Renders the weekly available-time schedule as a BMP image and
//! sets it as the desktop wallpaper. Read-only w.r.t. the system:
//! no firewall or service changes, only the user's own wallpaper.

use crate::config::AppConfig;
use crate::lang;
use crate::ui::font::create_ui_font;
use chrono::Local;
use std::path::PathBuf;
use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject,
    GetDC, GetDIBits, Rectangle, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW,
    BI_RGB, BITMAPFILEHEADER, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ, RGBQUAD,
    TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SystemParametersInfoW, SM_CXSCREEN, SM_CYSCREEN, SPIF_SENDCHANGE,
    SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

const DAY_MINUTES: u32 = 1440;

fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

fn fmt_hm(total: u32) -> String {
    if total >= DAY_MINUTES {
        return "24:00".to_string();
    }
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// Available (unblocked) ranges of a calendar day from raw (start, end)
/// minute slots. `start == end` means unused. Crossing slots spill into
/// the next morning via `prev` (yesterday's slots). Pure: unit-testable.
pub fn allowed_ranges_for_day(today: &[(u32, u32)], prev: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut blocked = [false; 1440];
    for m in 0..DAY_MINUTES {
        let mut b = false;
        for &(s, e) in today {
            if s == e {
                continue;
            }
            if s < e {
                if m >= s && m < e {
                    b = true;
                    break;
                }
            } else if m >= s {
                b = true;
                break;
            }
        }
        if !b {
            for &(s, e) in prev {
                if s == e {
                    continue;
                }
                if s > e && m < e {
                    b = true;
                    break;
                }
            }
        }
        blocked[m as usize] = b;
    }

    let mut out = Vec::new();
    let mut m = 0usize;
    while m < 1440 {
        if !blocked[m] {
            let start = m;
            while m < 1440 && !blocked[m] {
                m += 1;
            }
            out.push((start as u32, m as u32));
        } else {
            m += 1;
        }
    }
    out
}

fn day_slots(cfg: &AppConfig, idx: usize) -> Vec<(u32, u32)> {
    cfg.weekly_schedule[idx % 7]
        .active_slots()
        .iter()
        .map(|(s, e, _)| (*s, *e))
        .collect()
}

fn blocked_segments(cfg: &AppConfig, idx: usize) -> Vec<(u32, u32)> {
    // Invert allowed ranges back into blocked segments for bar drawing.
    let today = day_slots(cfg, idx);
    let prev = day_slots(cfg, (idx + 6) % 7);
    let allowed = allowed_ranges_for_day(&today, &prev);
    let mut segs = Vec::new();
    let mut cursor = 0u32;
    for (s, e) in allowed {
        if s > cursor {
            segs.push((cursor, s));
        }
        cursor = e;
    }
    if cursor < DAY_MINUTES {
        segs.push((cursor, DAY_MINUTES));
    }
    segs
}

fn describe_ranges(ranges: &[(u32, u32)]) -> String {
    if ranges.is_empty() {
        return lang::BLOCKED_ALL_DAY.to_string();
    }
    if ranges.len() == 1 && ranges[0] == (0, DAY_MINUTES) {
        return lang::AVAILABLE_ALL_DAY.to_string();
    }
    ranges
        .iter()
        .map(|(s, e)| format!("{} - {}", fmt_hm(*s), fmt_hm(*e)))
        .collect::<Vec<_>>()
        .join(",  ")
}

struct Canvas {
    mem_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    screen_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    bitmap: windows_sys::Win32::Graphics::Gdi::HBITMAP,
    width: i32,
    height: i32,
}

impl Canvas {
    unsafe fn create(width: i32, height: i32) -> Result<Self, String> {
        unsafe {
            let screen_dc = GetDC(null_mut());
            if screen_dc.is_null() {
                return Err("GetDC failed".to_string());
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return Err("CreateCompatibleDC failed".to_string());
            }
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err("CreateCompatibleBitmap failed".to_string());
            }
            SelectObject(mem_dc, bitmap as HGDIOBJ);
            Ok(Self {
                mem_dc,
                screen_dc,
                bitmap,
                width,
                height,
            })
        }
    }

    unsafe fn fill_rect(&self, left: i32, top: i32, right: i32, bottom: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(self.mem_dc, brush as HGDIOBJ);
            Rectangle(self.mem_dc, left, top, right, bottom);
            SelectObject(self.mem_dc, old);
            DeleteObject(brush as HGDIOBJ);
        }
    }

    unsafe fn text(
        &self,
        x: i32,
        y: i32,
        text: &str,
        color: u32,
        font: windows_sys::Win32::Graphics::Gdi::HFONT,
    ) {
        unsafe {
            let old_font = SelectObject(self.mem_dc, font as HGDIOBJ);
            SetTextColor(self.mem_dc, color);
            SetBkMode(self.mem_dc, TRANSPARENT as i32);
            let wide: Vec<u16> = text.encode_utf16().collect();
            TextOutW(self.mem_dc, x, y, wide.as_ptr(), wide.len() as i32);
            SelectObject(self.mem_dc, old_font);
        }
    }

    unsafe fn to_bmp_bytes(&self) -> Result<Vec<u8>, String> {
        unsafe {
            let stride = (((self.width * 24 + 31) / 32) * 4) as usize;
            let mut pixels = vec![0u8; stride * self.height as usize];
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width;
            bmi.bmiHeader.biHeight = self.height; // positive: bottom-up DIB
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 24;
            bmi.bmiHeader.biCompression = BI_RGB;
            bmi.bmiColors[0] = RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            };
            if GetDIBits(
                self.mem_dc,
                self.bitmap,
                0,
                self.height as u32,
                pixels.as_mut_ptr() as *mut _,
                &mut bmi,
                DIB_RGB_COLORS,
            ) == 0
            {
                return Err("GetDIBits failed".to_string());
            }

            let header = BITMAPFILEHEADER {
                bfType: 0x4D42,
                bfSize: (54 + pixels.len()) as u32,
                bfReserved1: 0,
                bfReserved2: 0,
                bfOffBits: 54,
            };
            let mut out = Vec::with_capacity(54 + pixels.len());
            let raw = std::slice::from_raw_parts(
                (&header as *const BITMAPFILEHEADER) as *const u8,
                std::mem::size_of::<BITMAPFILEHEADER>(),
            );
            out.extend_from_slice(raw);
            let raw_info = std::slice::from_raw_parts(
                (&bmi.bmiHeader as *const BITMAPINFOHEADER) as *const u8,
                std::mem::size_of::<BITMAPINFOHEADER>(),
            );
            out.extend_from_slice(raw_info);
            out.extend_from_slice(&pixels);
            Ok(out)
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.mem_dc);
            ReleaseDC(null_mut(), self.screen_dc);
        }
    }
}

fn draw_schedule(canvas: &Canvas, cfg: &AppConfig) {
    let w = canvas.width;
    let h = canvas.height;

    let bg = rgb(15, 23, 42);
    let track = rgb(30, 41, 59);
    let green = rgb(70, 180, 100);
    let red = rgb(220, 70, 70);
    let white = rgb(235, 240, 250);
    let gray = rgb(150, 160, 180);

    unsafe {
        canvas.fill_rect(0, 0, w, h, bg);

        let title_font = create_ui_font(h * 32 / 1000, true);
        let row_font = create_ui_font(h * 19 / 1000, false);
        let small_font = create_ui_font(h * 15 / 1000, false);

        let mx = w / 14;
        canvas.text(
            mx,
            h * 6 / 100,
            lang::VIEWER_TITLE,
            white,
            title_font,
        );
        let stamp = Local::now().format("Updated %Y-%m-%d %H:%M").to_string();
        canvas.text(mx, h * 6 / 100 + h * 38 / 1000, &stamp, gray, small_font);

        let bar_x0 = mx + w * 11 / 100;
        let text_x = w - mx - w * 30 / 100;
        let bar_x1 = text_x - 30;
        let rows_top = h * 22 / 100;
        let rows_h = h * 62 / 100;
        let row_h = rows_h / 7;
        let bar_h = row_h * 34 / 100;

        // Hour ticks above the first row.
        for hour in (0..=24).step_by(6) {
            let x = bar_x0 + (bar_x1 - bar_x0) * hour / 24;
            canvas.text(
                x - 10,
                rows_top - h * 24 / 1000,
                &format!("{:02}", hour),
                gray,
                small_font,
            );
        }

        for i in 0..7 {
            let row_y = rows_top + row_h * i as i32;
            let cy = row_y + row_h / 2;
            canvas.text(mx, cy - h * 11 / 1000, lang::WEEKDAY_FULL[i], white, row_font);

            let bar_y = cy - bar_h / 2;
            canvas.fill_rect(bar_x0, bar_y, bar_x1, bar_y + bar_h, track);
            canvas.fill_rect(bar_x0, bar_y, bar_x1, bar_y + bar_h, green);
            for (s, e) in blocked_segments(cfg, i) {
                let x0 = bar_x0 + (bar_x1 - bar_x0) * s as i32 / DAY_MINUTES as i32;
                let x1 = bar_x0 + (bar_x1 - bar_x0) * e.min(DAY_MINUTES) as i32 / DAY_MINUTES as i32;
                if x1 > x0 {
                    canvas.fill_rect(x0, bar_y, x1, bar_y + bar_h, red);
                }
            }

            let today = day_slots(cfg, i);
            let prev = day_slots(cfg, (i + 6) % 7);
            let label = describe_ranges(&allowed_ranges_for_day(&today, &prev));
            canvas.text(text_x, cy - h * 10 / 1000, &label, gray, small_font);
        }

        canvas.text(mx, h * 90 / 100, lang::VIEWER_NOTE, gray, small_font);

        DeleteObject(title_font as HGDIOBJ);
        DeleteObject(row_font as HGDIOBJ);
        DeleteObject(small_font as HGDIOBJ);
    }
}

pub fn wallpaper_path() -> PathBuf {
    let cfg_path = AppConfig::config_path();
    let dir = cfg_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join("schedule_wallpaper.bmp")
}

/// Renders the current schedule and sets it as the desktop wallpaper.
/// Returns the written image path.
pub fn apply_schedule_wallpaper(cfg: &AppConfig) -> Result<PathBuf, String> {
    let (w, h) = unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w <= 0 || h <= 0 {
            (1920, 1080)
        } else {
            (w, h)
        }
    };

    let canvas = unsafe { Canvas::create(w, h)? };
    draw_schedule(&canvas, cfg);
    let bytes = unsafe { canvas.to_bmp_bytes()? };

    let path = wallpaper_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    let wide: Vec<u16> = path
        .to_string_lossy()
        .chars()
        .chain(std::iter::once('\0'))
        .flat_map(|c| {
            let mut buf = [0u16; 2];
            c.encode_utf16(&mut buf).to_vec()
        })
        .collect();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide.as_ptr() as *mut _,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    };
    if ok == 0 {
        return Err("SystemParametersInfoW failed".to_string());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_slot_ranges() {
        // Block 00:00-07:00 -> available 07:00-24:00.
        let ranges = allowed_ranges_for_day(&[(0, 420)], &[]);
        assert_eq!(ranges, vec![(420, 1440)]);
    }

    #[test]
    fn test_two_slots_and_empty() {
        // Unused slot (start == end) is ignored.
        let ranges = allowed_ranges_for_day(&[(0, 420), (1320, 1380)], &[(0, 0)]);
        assert_eq!(ranges, vec![(420, 1320), (1380, 1440)]);
        // Fully open day (including an unused 00:00-00:00 slot).
        assert_eq!(allowed_ranges_for_day(&[], &[]), vec![(0, 1440)]);
        assert_eq!(allowed_ranges_for_day(&[(0, 0)], &[]), vec![(0, 1440)]);
        // Fully blocked day.
        assert!(allowed_ranges_for_day(&[(0, 1440)], &[]).is_empty());
    }

    #[test]
    fn test_crossing_spillover() {
        // Monday night 23:00-07:00 spills into Tuesday 00:00-07:00.
        let monday = vec![(1380, 420)];
        let tuesday_own: Vec<(u32, u32)> = vec![];
        let tue = allowed_ranges_for_day(&tuesday_own, &monday);
        assert_eq!(tue, vec![(420, 1440)]);
        // Monday itself keeps its whole morning open (belongs to Sunday).
        let sun: Vec<(u32, u32)> = vec![];
        let mon = allowed_ranges_for_day(&monday, &sun);
        assert_eq!(mon, vec![(0, 1380)]);
    }
}
