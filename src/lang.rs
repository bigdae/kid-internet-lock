//! UI language strings.
//!
//! English by default. Build with `--features ko` for a Korean UI:
//! `cargo build --release --features ko`.

// Shared by both binaries; each binary uses a subset.
#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Weekdays: Monday = 0 .. Sunday = 6
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const WEEKDAY_SHORT: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];
#[cfg(not(feature = "ko"))]
pub const WEEKDAY_SHORT: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[cfg(feature = "ko")]
pub const WEEKDAY_FULL: [&str; 7] = [
    "월요일",
    "화요일",
    "수요일",
    "목요일",
    "금요일",
    "토요일",
    "일요일",
];
#[cfg(not(feature = "ko"))]
pub const WEEKDAY_FULL: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

// ---------------------------------------------------------------------------
// Scheduler status (tray tooltip, settings status line)
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const SLOTS_NONE: &str = "없음";
#[cfg(not(feature = "ko"))]
pub const SLOTS_NONE: &str = "none";

#[cfg(feature = "ko")]
pub fn blocked_summary(src: &str, active: &str, all: &str, end: &str) -> String {
    format!("{src} {active} 차단 중 ({all}) ({end} 해제)")
}
#[cfg(not(feature = "ko"))]
pub fn blocked_summary(src: &str, active: &str, all: &str, end: &str) -> String {
    format!("Internet blocked - {src} {active} ({all}) (unblocks at {end})")
}

#[cfg(feature = "ko")]
pub fn blocked_tooltip(src: &str, all: &str, end: &str) -> String {
    format!("Kid Internet Lock\n상태: 🔴 차단 중 ({src} {all})\n해제: {end}")
}
#[cfg(not(feature = "ko"))]
pub fn blocked_tooltip(src: &str, all: &str, end: &str) -> String {
    format!("Kid Internet Lock\nStatus: 🔴 Blocked ({src} {all})\nUnblocks at: {end}")
}

#[cfg(feature = "ko")]
pub fn allowed_summary(today: &str, today_slots: &str, next: &str, next_slots: &str) -> String {
    format!("인터넷 허용 중 - {today} ({today_slots}) (다음 차단 {next} {next_slots})")
}
#[cfg(not(feature = "ko"))]
pub fn allowed_summary(today: &str, today_slots: &str, next: &str, next_slots: &str) -> String {
    format!("Internet allowed - {today} ({today_slots}) (next block {next} {next_slots})")
}

#[cfg(feature = "ko")]
pub fn allowed_tooltip(today: &str, next: &str, next_slots: &str) -> String {
    format!("Kid Internet Lock\n상태: 🟢 허용 중 ({today})\n다음 차단: {next} {next_slots}")
}
#[cfg(not(feature = "ko"))]
pub fn allowed_tooltip(today: &str, next: &str, next_slots: &str) -> String {
    format!("Kid Internet Lock\nStatus: 🟢 Allowed ({today})\nNext block: {next} {next_slots}")
}

#[cfg(feature = "ko")]
pub const FIREWALL_FAIL_SUMMARY_SUFFIX: &str = " ⚠️ 방화벽 규칙 적용 실패";
#[cfg(not(feature = "ko"))]
pub const FIREWALL_FAIL_SUMMARY_SUFFIX: &str = " ⚠️ Failed to apply firewall rule";

#[cfg(feature = "ko")]
pub const FIREWALL_FAIL_TOOLTIP_SUFFIX: &str =
    "\n⚠️ 방화벽 규칙을 적용하지 못했습니다 (보안 정책/권한 확인)";
#[cfg(not(feature = "ko"))]
pub const FIREWALL_FAIL_TOOLTIP_SUFFIX: &str =
    "\n⚠️ Failed to apply the firewall rule (check security policy/permissions)";

#[cfg(feature = "ko")]
pub const ALLOWED_NONE_SUMMARY: &str = "인터넷 허용 중 (차단 일정 없음)";
#[cfg(not(feature = "ko"))]
pub const ALLOWED_NONE_SUMMARY: &str = "Internet allowed (no weekday block scheduled)";

#[cfg(feature = "ko")]
pub const ALLOWED_NONE_TOOLTIP: &str =
    "Kid Internet Lock\n상태: 🟢 허용 중\n차단 일정 없음";
#[cfg(not(feature = "ko"))]
pub const ALLOWED_NONE_TOOLTIP: &str =
    "Kid Internet Lock\nStatus: 🟢 Allowed\nNo weekday block scheduled";

#[cfg(feature = "ko")]
pub const MANUAL_BLOCKED_SUMMARY: &str = "인터넷 수동 차단 중";
#[cfg(not(feature = "ko"))]
pub const MANUAL_BLOCKED_SUMMARY: &str = "Internet manually blocked";

#[cfg(feature = "ko")]
pub const MANUAL_BLOCKED_TOOLTIP: &str =
    "Kid Internet Lock\n상태: 🔴 수동 차단 중\n(수동 제어 활성)";
#[cfg(not(feature = "ko"))]
pub const MANUAL_BLOCKED_TOOLTIP: &str =
    "Kid Internet Lock\nStatus: 🔴 Manually Blocked\n(Manual control active)";

#[cfg(feature = "ko")]
pub const MANUAL_ALLOWED_SUMMARY: &str = "인터넷 수동 허용 중";
#[cfg(not(feature = "ko"))]
pub const MANUAL_ALLOWED_SUMMARY: &str = "Internet manually allowed";

#[cfg(feature = "ko")]
pub const MANUAL_ALLOWED_TOOLTIP: &str =
    "Kid Internet Lock\n상태: 🟢 수동 허용 중\n(수동 제어 활성)";
#[cfg(not(feature = "ko"))]
pub const MANUAL_ALLOWED_TOOLTIP: &str =
    "Kid Internet Lock\nStatus: 🟢 Manually Allowed\n(Manual control active)";

#[cfg(feature = "ko")]
pub fn temp_allow_summary(mins: i64) -> String {
    format!("임시 허용 중 (약 {mins}분 남음)")
}
#[cfg(not(feature = "ko"))]
pub fn temp_allow_summary(mins: i64) -> String {
    format!("Temporary allow active (about {mins} min left)")
}

#[cfg(feature = "ko")]
pub fn temp_allow_tooltip(mins: i64, until: &str) -> String {
    format!("Kid Internet Lock\n상태: 🟢 임시 허용 중\n남은 시간: 약 {mins}분 ({until}까지)")
}
#[cfg(not(feature = "ko"))]
pub fn temp_allow_tooltip(mins: i64, until: &str) -> String {
    format!("Kid Internet Lock\nStatus: 🟢 Temporary allow active\nRemaining: about {mins} min (until {until})")
}

// ---------------------------------------------------------------------------
// Tray menu + shared buttons
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const TRAY_STATUS_BLOCKED: &str = "Kid Internet Lock [🔴 인터넷 차단 중]";
#[cfg(not(feature = "ko"))]
pub const TRAY_STATUS_BLOCKED: &str = "Kid Internet Lock [🔴 Internet Blocked]";

#[cfg(feature = "ko")]
pub const TRAY_STATUS_ALLOWED: &str = "Kid Internet Lock [🟢 인터넷 허용 중]";
#[cfg(not(feature = "ko"))]
pub const TRAY_STATUS_ALLOWED: &str = "Kid Internet Lock [🟢 Internet Allowed]";

#[cfg(feature = "ko")]
pub const BTN_ALLOW_30: &str = "30분 허용";
#[cfg(not(feature = "ko"))]
pub const BTN_ALLOW_30: &str = "Allow 30 Minutes";

#[cfg(feature = "ko")]
pub const BTN_ALLOW_60: &str = "1시간 허용";
#[cfg(not(feature = "ko"))]
pub const BTN_ALLOW_60: &str = "Allow 1 Hour";

#[cfg(feature = "ko")]
pub const BTN_TEMP_CANCEL: &str = "임시 허용 취소";
#[cfg(not(feature = "ko"))]
pub const BTN_TEMP_CANCEL: &str = "Cancel Temporary Allow";

#[cfg(feature = "ko")]
pub const BTN_TOGGLE: &str = "지금 차단 / 차단 해제";
#[cfg(not(feature = "ko"))]
pub const BTN_TOGGLE: &str = "Block / Unblock Now";

#[cfg(feature = "ko")]
pub const MENU_SETTINGS: &str = "관리자 설정(S)...";
#[cfg(not(feature = "ko"))]
pub const MENU_SETTINGS: &str = "Admin Settings (S)...";

#[cfg(feature = "ko")]
pub const MENU_EXIT: &str = "종료(X)";
#[cfg(not(feature = "ko"))]
pub const MENU_EXIT: &str = "Exit (X)";

#[cfg(feature = "ko")]
pub const AUTH_TITLE_ADMIN: &str = "Kid Internet Lock - 관리자 인증";
#[cfg(not(feature = "ko"))]
pub const AUTH_TITLE_ADMIN: &str = "Kid Internet Lock - Admin Authentication";

#[cfg(feature = "ko")]
pub const AUTH_TITLE_SETTINGS: &str = "Kid Internet Lock - 설정 관리자 인증";
#[cfg(not(feature = "ko"))]
pub const AUTH_TITLE_SETTINGS: &str = "Kid Internet Lock - Settings Authentication";

#[cfg(feature = "ko")]
pub const AUTH_TITLE_EXIT: &str = "Kid Internet Lock - 종료 관리자 인증";
#[cfg(not(feature = "ko"))]
pub const AUTH_TITLE_EXIT: &str = "Kid Internet Lock - Exit Authentication";

#[cfg(feature = "ko")]
pub fn auth_title(action: &str) -> String {
    format!("{action} - 관리자 인증")
}
#[cfg(not(feature = "ko"))]
pub fn auth_title(action: &str) -> String {
    format!("{action} - Admin Authentication")
}

// ---------------------------------------------------------------------------
// Settings dialog
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const TITLE_SETTINGS: &str = "Kid Internet Lock - 관리자 설정";
#[cfg(not(feature = "ko"))]
pub const TITLE_SETTINGS: &str = "Kid Internet Lock - Admin Settings";

#[cfg(feature = "ko")]
pub const GRP_SCHEDULE: &str = " 🕒 차단 스케줄 (요일별, 시간대 2개) ";
#[cfg(not(feature = "ko"))]
pub const GRP_SCHEDULE: &str = " 🕒 Block Schedule (by weekday, 2 slots) ";

#[cfg(feature = "ko")]
pub const BATCH1_LABEL: &str = "일괄 1차:";
#[cfg(not(feature = "ko"))]
pub const BATCH1_LABEL: &str = "Batch slot 1:";

#[cfg(feature = "ko")]
pub const BATCH2_LABEL: &str = "일괄 2차:";
#[cfg(not(feature = "ko"))]
pub const BATCH2_LABEL: &str = "Batch slot 2:";

#[cfg(feature = "ko")]
pub const BTN_COPY_SLOT1: &str = "1차 → 전체 요일에 복사";
#[cfg(not(feature = "ko"))]
pub const BTN_COPY_SLOT1: &str = "Copy slot 1 to all days";

#[cfg(feature = "ko")]
pub const BTN_COPY_SLOT2: &str = "2차 → 전체 요일에 복사";
#[cfg(not(feature = "ko"))]
pub const BTN_COPY_SLOT2: &str = "Copy slot 2 to all days";

#[cfg(feature = "ko")]
pub const HDR_DAY: &str = "요일";
#[cfg(not(feature = "ko"))]
pub const HDR_DAY: &str = "Day";

#[cfg(feature = "ko")]
pub const HDR_SLOT1: &str = "1차 차단 (시작 ~ 종료)";
#[cfg(not(feature = "ko"))]
pub const HDR_SLOT1: &str = "Slot 1 block (start - end)";

#[cfg(feature = "ko")]
pub const HDR_SLOT2: &str = "2차 차단 (시작 ~ 종료, 미사용 시 00:00 ~ 00:00)";
#[cfg(not(feature = "ko"))]
pub const HDR_SLOT2: &str = "Slot 2 block (start - end, 00:00 - 00:00 when unused)";

#[cfg(feature = "ko")]
pub const ROW_HINT: &str = "차단";
#[cfg(not(feature = "ko"))]
pub const ROW_HINT: &str = "block";

#[cfg(feature = "ko")]
pub const NOTE_SCHEDULE: &str =
    "※ 요일 체크 해제 = 종일 허용 / 시작 = 종료 이면 해당 시간대 사용 안 함 / 자정을 넘기면 다음날 새벽까지 이어짐";
#[cfg(not(feature = "ko"))]
pub const NOTE_SCHEDULE: &str =
    "Uncheck a day = allow all day / Start = end disables that slot / Crossing midnight continues into next morning";

#[cfg(feature = "ko")]
pub const GRP_TEMP: &str = " ⚡ 임시 허용 및 수동 제어 ";
#[cfg(not(feature = "ko"))]
pub const GRP_TEMP: &str = " ⚡ Temporary Allow & Manual Control ";

#[cfg(feature = "ko")]
pub const GRP_PASSWORD: &str = " 🔑 관리자 비밀번호 변경 ";
#[cfg(not(feature = "ko"))]
pub const GRP_PASSWORD: &str = " 🔑 Change Admin Password ";

#[cfg(feature = "ko")]
pub const LBL_CURRENT_PWD: &str = "현재 비밀번호:";
#[cfg(not(feature = "ko"))]
pub const LBL_CURRENT_PWD: &str = "Current password:";

#[cfg(feature = "ko")]
pub const LBL_NEW_PWD: &str = "새 비밀번호:";
#[cfg(not(feature = "ko"))]
pub const LBL_NEW_PWD: &str = "New password:";

#[cfg(feature = "ko")]
pub const LBL_CONFIRM_PWD: &str = "새 비밀번호 확인:";
#[cfg(not(feature = "ko"))]
pub const LBL_CONFIRM_PWD: &str = "Confirm new password:";

#[cfg(feature = "ko")]
pub const BTN_CHANGE_PWD: &str = "비밀번호 변경";
#[cfg(not(feature = "ko"))]
pub const BTN_CHANGE_PWD: &str = "Change Password";

#[cfg(feature = "ko")]
pub const PWD_ERR_CURRENT: &str = "현재 비밀번호가 틀렸습니다.";
#[cfg(not(feature = "ko"))]
pub const PWD_ERR_CURRENT: &str = "The current password is incorrect.";

#[cfg(feature = "ko")]
pub const PWD_ERR_EMPTY: &str = "새 비밀번호를 입력하세요.";
#[cfg(not(feature = "ko"))]
pub const PWD_ERR_EMPTY: &str = "Please enter a new password.";

#[cfg(feature = "ko")]
pub const PWD_ERR_MISMATCH: &str = "새 비밀번호 확인이 일치하지 않습니다.";
#[cfg(not(feature = "ko"))]
pub const PWD_ERR_MISMATCH: &str = "The new password confirmation does not match.";

#[cfg(feature = "ko")]
pub const PWD_OK: &str = "비밀번호가 변경되었습니다.";
#[cfg(not(feature = "ko"))]
pub const PWD_OK: &str = "Password changed successfully.";

#[cfg(feature = "ko")]
pub fn save_failed_msg(detail: &str) -> String {
    format!("저장 실패: {detail}")
}
#[cfg(not(feature = "ko"))]
pub fn save_failed_msg(detail: &str) -> String {
    format!("Save failed: {detail}")
}

#[cfg(feature = "ko")]
pub const GRP_STARTUP: &str = " ⚙️ 시작 프로그램 ";
#[cfg(not(feature = "ko"))]
pub const GRP_STARTUP: &str = " ⚙️ Startup ";

#[cfg(feature = "ko")]
pub const CHK_AUTOSTART: &str = "Windows 로그온 시 백그라운드에서 자동 실행";
#[cfg(not(feature = "ko"))]
pub const CHK_AUTOSTART: &str = "Run automatically in the background at Windows logon";

#[cfg(feature = "ko")]
pub const BTN_SAVE: &str = "설정 저장";
#[cfg(not(feature = "ko"))]
pub const BTN_SAVE: &str = "Save Settings";

#[cfg(feature = "ko")]
pub const BTN_CLOSE: &str = "닫기";
#[cfg(not(feature = "ko"))]
pub const BTN_CLOSE: &str = "Close";

#[cfg(feature = "ko")]
pub const TITLE_SAVED: &str = "설정 저장됨";
#[cfg(not(feature = "ko"))]
pub const TITLE_SAVED: &str = "Settings Saved";

#[cfg(feature = "ko")]
pub const MSG_SAVED: &str = "설정이 저장되었습니다.";
#[cfg(not(feature = "ko"))]
pub const MSG_SAVED: &str = "Settings saved successfully.";

#[cfg(feature = "ko")]
pub const TITLE_INPUT_ERROR: &str = "입력 오류";
#[cfg(not(feature = "ko"))]
pub const TITLE_INPUT_ERROR: &str = "Input Error";

#[cfg(feature = "ko")]
pub const SLOT1_NAME: &str = "1차";
#[cfg(not(feature = "ko"))]
pub const SLOT1_NAME: &str = "Slot 1";

#[cfg(feature = "ko")]
pub const SLOT2_NAME: &str = "2차";
#[cfg(not(feature = "ko"))]
pub const SLOT2_NAME: &str = "Slot 2";

#[cfg(feature = "ko")]
pub fn batch_error(slot: &str) -> String {
    format!("일괄 {slot}: 시 00-23, 분 00-59 로 입력해 주세요.")
}
#[cfg(not(feature = "ko"))]
pub fn batch_error(slot: &str) -> String {
    format!("Batch {slot}: please enter hours 00-23 and minutes 00-59.")
}

#[cfg(feature = "ko")]
pub fn weekly_error(day: &str) -> String {
    format!("{day}: 시간은 시 00-23, 분 00-59 로 입력해 주세요. (2차 미사용 시 00:00-00:00)")
}
#[cfg(not(feature = "ko"))]
pub fn weekly_error(day: &str) -> String {
    format!("{day}: please enter hours 00-23 and minutes 00-59. (Leave slot 2 as 00:00-00:00 when unused.)")
}

#[cfg(feature = "ko")]
pub fn status_line(summary: &str) -> String {
    format!("현재 상태: {summary}")
}
#[cfg(not(feature = "ko"))]
pub fn status_line(summary: &str) -> String {
    format!("Current status: {summary}")
}

// ---------------------------------------------------------------------------
// Password dialog
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const PWD_PROMPT_LABEL: &str = "관리자 비밀번호를 입력하세요:";
#[cfg(not(feature = "ko"))]
pub const PWD_PROMPT_LABEL: &str = "Enter the admin password:";

#[cfg(feature = "ko")]
pub const PWD_INCORRECT: &str = "비밀번호가 틀렸습니다.";
#[cfg(not(feature = "ko"))]
pub const PWD_INCORRECT: &str = "Incorrect password.";

#[cfg(feature = "ko")]
pub const BTN_OK: &str = "확인";
#[cfg(not(feature = "ko"))]
pub const BTN_OK: &str = "OK";

#[cfg(feature = "ko")]
pub const BTN_CANCEL: &str = "취소";
#[cfg(not(feature = "ko"))]
pub const BTN_CANCEL: &str = "Cancel";

// ---------------------------------------------------------------------------
// Startup notice dialogs (main.rs)
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const TITLE_ADMIN_REQUIRED: &str = "관리자 권한 필요";
#[cfg(not(feature = "ko"))]
pub const TITLE_ADMIN_REQUIRED: &str = "Administrator Rights Required";

#[cfg(feature = "ko")]
pub const MSG_ADMIN_REQUIRED: &str =
    "Kid Internet Lock은 Windows 방화벽 제어를 위해 관리자 권한이 필요합니다.\n'관리자 권한으로 실행'을 선택하세요.";
#[cfg(not(feature = "ko"))]
pub const MSG_ADMIN_REQUIRED: &str =
    "Kid Internet Lock needs administrator rights to control Windows Firewall.\nPlease choose 'Run as administrator'.";

#[cfg(feature = "ko")]
pub const TITLE_NOTICE: &str = "알림";
#[cfg(not(feature = "ko"))]
pub const TITLE_NOTICE: &str = "Notice";

#[cfg(feature = "ko")]
pub const MSG_ALREADY_RUNNING: &str =
    "Kid Internet Lock이 이미 실행 중입니다.\n작업 표시줄 오른쪽 트레이 아이콘을 확인하세요.";
#[cfg(not(feature = "ko"))]
pub const MSG_ALREADY_RUNNING: &str =
    "Kid Internet Lock is already running.\nPlease check the tray icon in the bottom-right corner of the taskbar.";

#[cfg(feature = "ko")]
pub const TITLE_ERROR: &str = "오류";
#[cfg(not(feature = "ko"))]
pub const TITLE_ERROR: &str = "Error";

#[cfg(feature = "ko")]
pub fn err_running(detail: &str) -> String {
    format!("프로그램 실행 중 오류가 발생했습니다:\n{detail}")
}
#[cfg(not(feature = "ko"))]
pub fn err_running(detail: &str) -> String {
    format!("An error occurred while running the application:\n{detail}")
}

// ---------------------------------------------------------------------------
// Kid schedule viewer
// ---------------------------------------------------------------------------

#[cfg(feature = "ko")]
pub const VIEWER_TITLE: &str = "인터넷 사용 가능 시간";
#[cfg(not(feature = "ko"))]
pub const VIEWER_TITLE: &str = "Internet Time - Weekly Schedule";

#[cfg(feature = "ko")]
pub const VIEWER_GROUP: &str = " 인터넷 사용 가능 시간 ";
#[cfg(not(feature = "ko"))]
pub const VIEWER_GROUP: &str = " Internet available times ";

#[cfg(feature = "ko")]
pub const AVAILABLE_ALL_DAY: &str = "종일 사용 가능";
#[cfg(not(feature = "ko"))]
pub const AVAILABLE_ALL_DAY: &str = "Available all day";

#[cfg(feature = "ko")]
pub const BLOCKED_ALL_DAY: &str = "종일 차단";
#[cfg(not(feature = "ko"))]
pub const BLOCKED_ALL_DAY: &str = "Blocked all day";

#[cfg(feature = "ko")]
pub fn viewer_available(day: &str, clock: &str, until: &str) -> String {
    format!("{day} {clock} - 현재 사용 가능 ({until}까지)")
}
#[cfg(not(feature = "ko"))]
pub fn viewer_available(day: &str, clock: &str, until: &str) -> String {
    format!("{day} {clock}  -  Available now (until {until})")
}

#[cfg(feature = "ko")]
pub fn viewer_blocked(day: &str, clock: &str, from: &str) -> String {
    format!("{day} {clock} - 현재 차단 중 ({from}부터 사용 가능)")
}
#[cfg(not(feature = "ko"))]
pub fn viewer_blocked(day: &str, clock: &str, from: &str) -> String {
    format!("{day} {clock}  -  Blocked now (available from {from})")
}

#[cfg(feature = "ko")]
pub const VIEWER_ALL_WEEK_AVAILABLE: &str = "인터넷 주간 종일 사용 가능";
#[cfg(not(feature = "ko"))]
pub const VIEWER_ALL_WEEK_AVAILABLE: &str = "Internet available all week";

#[cfg(feature = "ko")]
pub const VIEWER_ALL_WEEK_BLOCKED: &str = "인터넷 주간 종일 차단";
#[cfg(not(feature = "ko"))]
pub const VIEWER_ALL_WEEK_BLOCKED: &str = "Internet blocked all week";

#[cfg(feature = "ko")]
pub const VIEWER_NOTE: &str = "위 시간에 인터넷을 사용할 수 있습니다. 나머지 시간은 차단됩니다.";
#[cfg(not(feature = "ko"))]
pub const VIEWER_NOTE: &str =
    "The times above are when the internet works. Everything else is blocked.";
