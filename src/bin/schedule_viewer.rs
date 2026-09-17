#![windows_subsystem = "windows"]
#![allow(dead_code)]

//! Kid-facing read-only viewer: today-first clock timetable.
//! No admin rights, no firewall changes, no tray icon — just a closable window.

#[path = "../config.rs"]
mod config;
#[path = "../firewall.rs"]
mod firewall;
#[path = "../lang.rs"]
mod lang;
#[path = "../scheduler.rs"]
mod scheduler;
#[path = "../ui/font.rs"]
mod font;

use chrono::{Datelike, Local, Timelike};
use config::AppConfig;
use font::{create_ui_font, set_control_fonts};
use lang::WEEKDAY_FULL;
use std::mem::zeroed;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, InvalidateRect, LineTo, MoveToEx,
    PAINTSTRUCT, Polygon, Rectangle, SelectObject, SetBkMode, SetTextColor, TextOutW,
    COLOR_BTNFACE, HBRUSH, HDC, HGDIOBJ, HFONT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, GetWindowLongPtrW, KillTimer, LoadCursorW, PostQuitMessage,
    RegisterClassExW, SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateMessage, BS_DEFPUSHBUTTON, BS_GROUPBOX, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA,
    IDC_ARROW, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_PAINT, WM_SETFONT, WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

const ID_TIMER_REFRESH: usize = 1;
const ID_BTN_CLOSE: usize = 110;
const REFRESH_MS: u32 = 30_000;

// Timeline bar geometry (client coordinates). Must match control layout in main().
const BAR_X0: i32 = 24;
const BAR_X1: i32 = 656;
const BAR_Y: i32 = 112;
const BAR_H: i32 = 40;
const HOUR_LABEL_Y: i32 = 88;
const LEGEND_Y: i32 = 162;

struct ViewerContext {
    status: HWND,
    today_title: HWND,
    today_ranges: HWND,
    tomorrow: HWND,
    rows: [HWND; 7],
    font_normal: HFONT,
    font_bold: HFONT,
}

fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

fn fmt_hm(total: u32) -> String {
    if total >= 1440 {
        return "24:00".to_string();
    }
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// Blocked minutes for a calendar day (Monday = 0 .. Sunday = 6),
/// including spillover from the previous day's crossing slots.
/// Mirrors the scheduler's block decision without touching the firewall.
fn blocked_grid(cfg: &AppConfig, day_idx: usize) -> [bool; 1440] {
    let today = &cfg.weekly_schedule[day_idx % 7];
    let prev = &cfg.weekly_schedule[(day_idx + 6) % 7];
    let today_slots = today.active_slots();
    let prev_slots = prev.active_slots();
    let mut grid = [false; 1440];
    for m in 0..1440u32 {
        let mut blocked = false;
        for (s, e, _) in today_slots.iter() {
            if *s < *e {
                if m >= *s && m < *e {
                    blocked = true;
                    break;
                }
            } else if m >= *s {
                blocked = true;
                break;
            }
        }
        if !blocked {
            for (s, e, _) in prev_slots.iter() {
                if *s > *e && m < *e {
                    blocked = true;
                    break;
                }
            }
        }
        grid[m as usize] = blocked;
    }
    grid
}

fn allowed_ranges(grid: &[bool; 1440]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut m = 0usize;
    while m < 1440 {
        if !grid[m] {
            let start = m;
            while m < 1440 && !grid[m] {
                m += 1;
            }
            out.push((start as u32, m as u32));
        } else {
            m += 1;
        }
    }
    out
}

fn describe_day(cfg: &AppConfig, day_idx: usize) -> String {
    let ranges = allowed_ranges(&blocked_grid(cfg, day_idx));
    if ranges.is_empty() {
        return lang::BLOCKED_ALL_DAY.to_string();
    }
    if ranges.len() == 1 && ranges[0] == (0, 1440) {
        return lang::AVAILABLE_ALL_DAY.to_string();
    }
    ranges
        .iter()
        .map(|(s, e)| format!("{} - {}", fmt_hm(*s), fmt_hm(*e)))
        .collect::<Vec<_>>()
        .join(",  ")
}

fn today_date_str(now: chrono::DateTime<Local>) -> String {
    #[cfg(feature = "ko")]
    {
        format!("{}월 {}일", now.month(), now.day())
    }
    #[cfg(not(feature = "ko"))]
    {
        now.format("%b %d").to_string()
    }
}

fn today_title_text() -> String {
    let now = Local::now();
    let idx = now.weekday().num_days_from_monday() as usize;
    lang::viewer_today_title(&today_date_str(now), WEEKDAY_FULL[idx])
}

fn today_ranges_text(cfg: &AppConfig) -> String {
    let now = Local::now();
    let idx = now.weekday().num_days_from_monday() as usize;
    lang::viewer_today_ranges(&describe_day(cfg, idx))
}

fn tomorrow_text(cfg: &AppConfig) -> String {
    let now = Local::now();
    let idx = now.weekday().num_days_from_monday() as usize;
    let tm = (idx + 1) % 7;
    lang::viewer_tomorrow(WEEKDAY_FULL[tm], &describe_day(cfg, tm))
}

/// Headline like "Tue 21:30 - Available (until 23:00)".
fn current_status(cfg: &AppConfig) -> String {
    let now = Local::now();
    let idx = now.weekday().num_days_from_monday() as usize;
    let cur = now.hour() * 60 + now.minute();
    let grids: Vec<[bool; 1440]> = (0..7).map(|d| blocked_grid(cfg, (idx + d) % 7)).collect();
    let blocked_now = grids[0][cur as usize];

    let mut change: Option<(usize, u32, bool)> = None;
    'search: for days_ahead in 0..8usize {
        let grid = &grids[days_ahead % 7];
        let from = if days_ahead == 0 {
            (cur as usize) + 1
        } else {
            0
        };
        for m in from..1440 {
            if grid[m] != blocked_now {
                change = Some((days_ahead, m as u32, grid[m]));
                break 'search;
            }
        }
    }

    let clock = now.format("%H:%M").to_string();
    let day_name = WEEKDAY_FULL[idx];
    let at = |days_ahead: usize, minute: u32| -> String {
        if days_ahead == 0 {
            fmt_hm(minute)
        } else {
            format!("{} {}", WEEKDAY_FULL[(idx + days_ahead) % 7], fmt_hm(minute))
        }
    };

    match (blocked_now, change) {
        (false, Some((da, m, _))) => lang::viewer_available(day_name, &clock, &at(da, m)),
        (true, Some((da, m, _))) => lang::viewer_blocked(day_name, &clock, &at(da, m)),
        (false, None) => lang::VIEWER_ALL_WEEK_AVAILABLE.to_string(),
        (true, None) => lang::VIEWER_ALL_WEEK_BLOCKED.to_string(),
    }
}

unsafe fn fill_rect_hdc(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        if brush.is_null() {
            return;
        }
        let old = SelectObject(hdc, brush as HGDIOBJ);
        Rectangle(hdc, left, top, right, bottom);
        SelectObject(hdc, old);
        DeleteObject(brush as HGDIOBJ);
    }
}

unsafe fn text_out_hdc(hdc: HDC, x: i32, y: i32, s: &str, color: u32, font: HFONT) {
    unsafe {
        let old_font = SelectObject(hdc, font as HGDIOBJ);
        SetTextColor(hdc, color);
        SetBkMode(hdc, TRANSPARENT as i32);
        let wide: Vec<u16> = s.encode_utf16().collect();
        TextOutW(hdc, x, y, wide.as_ptr(), wide.len() as i32);
        SelectObject(hdc, old_font);
    }
}

/// Paints the 24-hour clock timetable for today:
/// green = available, red = blocked, yellow marker = now.
unsafe fn paint_timeline(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.is_null() {
            return;
        }

        let cfg = AppConfig::load();
        let now = Local::now();
        let today = now.weekday().num_days_from_monday() as usize;
        let grid = blocked_grid(&cfg, today);
        let ranges = allowed_ranges(&grid);
        let now_min = now.hour() * 60 + now.minute();

        let green = rgb(70, 180, 100);
        let red = rgb(220, 70, 70);
        let yellow = rgb(255, 190, 0);
        let black = rgb(30, 30, 30);
        let gray = rgb(110, 110, 110);

        let small_font = create_ui_font(12, false);

        // Base: blocked, then overlay available ranges.
        fill_rect_hdc(hdc, BAR_X0, BAR_Y, BAR_X1, BAR_Y + BAR_H, red);
        for (s, e) in ranges.iter() {
            let x0 = BAR_X0 + (BAR_X1 - BAR_X0) * (*s as i32) / 1440;
            let x1 = BAR_X0 + (BAR_X1 - BAR_X0) * (*e as i32).min(1440) / 1440;
            if x1 > x0 {
                fill_rect_hdc(hdc, x0, BAR_Y, x1, BAR_Y + BAR_H, green);
            }
        }

        // Hour ticks + labels every 3 hours.
        for h in (0..=24).step_by(3) {
            let x = BAR_X0 + (BAR_X1 - BAR_X0) * h / 24;
            MoveToEx(hdc, x, BAR_Y + BAR_H, null_mut());
            LineTo(hdc, x, BAR_Y + BAR_H + 5);
            let label = format!("{:02}", h);
            let lx = (x - 10).clamp(BAR_X0, BAR_X1 - 20);
            text_out_hdc(hdc, lx, HOUR_LABEL_Y, &label, gray, small_font);
        }

        // Now marker: yellow bar + triangle on top.
        let x_now = BAR_X0 + (BAR_X1 - BAR_X0) * (now_min as i32) / 1440;
        let xn = x_now.clamp(BAR_X0, BAR_X1);
        fill_rect_hdc(hdc, xn - 1, BAR_Y - 4, xn + 2, BAR_Y + BAR_H, yellow);
        {
            let brush = CreateSolidBrush(yellow);
            if !brush.is_null() {
                let old = SelectObject(hdc, brush as HGDIOBJ);
                let pts = [
                    POINT {
                        x: xn - 7,
                        y: BAR_Y - 14,
                    },
                    POINT {
                        x: xn + 7,
                        y: BAR_Y - 14,
                    },
                    POINT { x: xn, y: BAR_Y - 4 },
                ];
                Polygon(hdc, pts.as_ptr(), 3);
                SelectObject(hdc, old);
                DeleteObject(brush as HGDIOBJ);
            }
        }

        // Legend below the bar.
        let ly = LEGEND_Y;
        fill_rect_hdc(hdc, BAR_X0, ly, BAR_X0 + 18, ly + 14, green);
        text_out_hdc(
            hdc,
            BAR_X0 + 22,
            ly - 3,
            lang::VIEWER_LEGEND_AVAILABLE,
            black,
            small_font,
        );
        fill_rect_hdc(hdc, BAR_X0 + 130, ly, BAR_X0 + 148, ly + 14, red);
        text_out_hdc(
            hdc,
            BAR_X0 + 152,
            ly - 3,
            lang::VIEWER_LEGEND_BLOCKED,
            black,
            small_font,
        );
        fill_rect_hdc(hdc, BAR_X0 + 230, ly, BAR_X0 + 248, ly + 14, yellow);
        let now_label = format!("{} {:02}:{:02}", lang::VIEWER_LEGEND_NOW, now.hour(), now.minute());
        text_out_hdc(hdc, BAR_X0 + 252, ly - 3, &now_label, black, small_font);

        DeleteObject(small_font as HGDIOBJ);
        EndPaint(hwnd, &ps);
    }
}

unsafe fn set_text(hwnd: HWND, s: &str) {
    unsafe {
        let wide: Vec<u16> = format!("{}\0", s).encode_utf16().collect();
        SetWindowTextW(hwnd, wide.as_ptr());
    }
}

unsafe fn refresh(ctx: &ViewerContext, hwnd: HWND) {
    unsafe {
        let cfg = AppConfig::load();
        let now = Local::now();
        let today = now.weekday().num_days_from_monday() as usize;

        set_text(ctx.status, &current_status(&cfg));
        set_text(ctx.today_title, &today_title_text());
        set_text(ctx.today_ranges, &today_ranges_text(&cfg));
        set_text(ctx.tomorrow, &tomorrow_text(&cfg));

        for i in 0..7 {
            let marker = if i == today { "> " } else { "    " };
            let line = format!("{}{}: {}", marker, WEEKDAY_FULL[i], describe_day(&cfg, i));
            set_text(ctx.rows[i], &line);
            // Emphasize today so the eye lands there first.
            let f = if i == today {
                ctx.font_bold
            } else {
                ctx.font_normal
            };
            SendMessageW(ctx.rows[i], WM_SETFONT, f as _, 1);
        }

        // Repaint the clock timetable (now-marker + colors).
        InvalidateRect(hwnd, null_mut(), 1);
    }
}

unsafe extern "system" fn viewer_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ctx_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ViewerContext;
        match msg {
            WM_PAINT => {
                paint_timeline(hwnd);
                0
            }
            WM_TIMER => {
                if wparam == ID_TIMER_REFRESH && !ctx_ptr.is_null() {
                    refresh(&*ctx_ptr, hwnd);
                }
                0
            }
            WM_COMMAND => {
                let id = (wparam & 0xFFFF) as usize;
                if id == ID_BTN_CLOSE {
                    DestroyWindow(hwnd);
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, ID_TIMER_REFRESH);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn main() {
    unsafe {
        let class_name: Vec<u16> = "KidInternetLock_ScheduleViewer\0".encode_utf16().collect();
        let title: Vec<u16> = format!("{}\0", lang::VIEWER_TITLE).encode_utf16().collect();
        let hinstance = GetModuleHandleW(null_mut());

        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(viewer_wnd_proc);
        wc.hInstance = hinstance;
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassExW(&wc);

        let width = 700;
        let height = 710;
        let x = (GetSystemMetrics(SM_CXSCREEN) - width) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - height) / 2;

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
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

        let font = create_ui_font(14, false);
        let font_bold = create_ui_font(15, true);
        let font_today_title = create_ui_font(21, true);
        let font_ranges = create_ui_font(16, true);
        let font_small = create_ui_font(13, false);

        let static_class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let btn_class: Vec<u16> = "BUTTON\0".encode_utf16().collect();

        let status = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            12,
            632,
            26,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        // Big "오늘 9월 18일 목요일" headline — no weekday hunting.
        let today_title = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            42,
            632,
            32,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        // Timeline bar itself is painted in WM_PAINT between y=88..180.

        let today_ranges = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            192,
            632,
            28,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let tomorrow = CreateWindowExW(
            0,
            static_class.as_ptr(),
            [0u16].as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            222,
            632,
            24,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let grp_title: Vec<u16> = format!("{}\0", lang::VIEWER_GROUP).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            grp_title.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
            16,
            252,
            648,
            316,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let mut rows: [HWND; 7] = [null_mut(); 7];
        for i in 0..7 {
            rows[i] = CreateWindowExW(
                0,
                static_class.as_ptr(),
                [0u16].as_ptr(),
                WS_CHILD | WS_VISIBLE,
                36,
                278 + (i as i32) * 38,
                610,
                24,
                hwnd,
                null_mut(),
                hinstance,
                null_mut(),
            );
        }

        let note: Vec<u16> = format!("{}\0", lang::VIEWER_NOTE).encode_utf16().collect();
        CreateWindowExW(
            0,
            static_class.as_ptr(),
            note.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            24,
            576,
            632,
            24,
            hwnd,
            null_mut(),
            hinstance,
            null_mut(),
        );

        let close_txt: Vec<u16> = format!("{}\0", lang::BTN_CLOSE).encode_utf16().collect();
        CreateWindowExW(
            0,
            btn_class.as_ptr(),
            close_txt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON as u32),
            540,
            608,
            116,
            36,
            hwnd,
            ID_BTN_CLOSE as _,
            hinstance,
            null_mut(),
        );

        set_control_fonts(hwnd, font);
        SendMessageW(status, WM_SETFONT, font_bold as _, 1);
        SendMessageW(today_title, WM_SETFONT, font_today_title as _, 1);
        SendMessageW(today_ranges, WM_SETFONT, font_ranges as _, 1);
        SendMessageW(tomorrow, WM_SETFONT, font_small as _, 1);

        let ctx = Box::into_raw(Box::new(ViewerContext {
            status,
            today_title,
            today_ranges,
            tomorrow,
            rows,
            font_normal: font,
            font_bold,
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);

        refresh(&*ctx, hwnd);
        SetTimer(hwnd, ID_TIMER_REFRESH, REFRESH_MS, None);

        ShowWindow(hwnd, SW_SHOW);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::AppState;
    use chrono::TimeZone;

    fn sample_cfg() -> AppConfig {
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
        // Monday: 00:00-07:00 and 22:00-23:00.
        let mon = &mut cfg.weekly_schedule[0];
        mon.enabled = true;
        mon.start_hour = 0;
        mon.start_minute = 0;
        mon.end_hour = 7;
        mon.end_minute = 0;
        mon.start2_hour = 22;
        mon.start2_minute = 0;
        mon.end2_hour = 23;
        mon.end2_minute = 0;
        // Wednesday: 23:00-06:00 crossing plus 12:00-13:00.
        let wed = &mut cfg.weekly_schedule[2];
        wed.enabled = true;
        wed.start_hour = 23;
        wed.start_minute = 0;
        wed.end_hour = 6;
        wed.end_minute = 0;
        wed.start2_hour = 12;
        wed.start2_minute = 0;
        wed.end2_hour = 13;
        wed.end2_minute = 0;
        // Sunday: second slot only, crossing 22:30-01:30.
        let sun = &mut cfg.weekly_schedule[6];
        sun.enabled = true;
        sun.start2_hour = 22;
        sun.start2_minute = 30;
        sun.end2_hour = 1;
        sun.end2_minute = 30;
        cfg
    }

    #[test]
    fn viewer_grid_matches_scheduler_all_week() {
        let cfg = sample_cfg();
        let state = AppState::new(cfg.clone());
        // 2026-09-14 is a Monday.
        for day_offset in 0..7u32 {
            for minute in (0..1440u32).step_by(7) {
                let dt = Local
                    .with_ymd_and_hms(2026, 9, 14 + day_offset, minute / 60, minute % 60, 0)
                    .unwrap();
                assert_eq!(
                    blocked_grid(&cfg, day_offset as usize)[minute as usize],
                    state.is_time_in_schedule(dt),
                    "day {} minute {}",
                    day_offset,
                    minute
                );
            }
        }
    }

    #[test]
    fn viewer_day_descriptions() {
        let cfg = sample_cfg();
        assert_eq!(
            describe_day(&cfg, 0),
            "07:00 - 22:00,  23:00 - 24:00"
        );
        // Tuesday disabled and no spillover into it.
        assert_eq!(describe_day(&cfg, 1), lang::AVAILABLE_ALL_DAY);
        assert_eq!(
            describe_day(&cfg, 2),
            "00:00 - 12:00,  13:00 - 23:00"
        );
    }

    #[test]
    fn viewer_today_text_wraps_description() {
        let cfg = sample_cfg();
        // Monday description embedded in both helpers.
        let mon_desc = describe_day(&cfg, 0);
        assert!(lang::viewer_today_ranges(&mon_desc).contains(&mon_desc));
        assert!(lang::viewer_tomorrow("X", &mon_desc).contains(&mon_desc));
        assert!(!today_date_str(Local::now()).is_empty());
    }
}
