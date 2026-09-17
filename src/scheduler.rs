use chrono::{DateTime, Datelike, Local, Timelike};
use crate::config::AppConfig;
use crate::firewall::FirewallManager;
use crate::lang::{self, WEEKDAY_FULL, WEEKDAY_SHORT};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSource {
    /// Non-crossing window active today (e.g. 01:00-07:00).
    Today { index: usize, slot: usize },
    /// Crossing window late part active today (e.g. 23:30, part of 23:30-06:30).
    TodayLate { index: usize, slot: usize },
    /// Previous day's crossing window spilling into this morning.
    YesterdaySpillover { yesterday: usize, slot: usize },
}

fn format_slot(sh: u32, sm: u32, eh: u32, em: u32) -> String {
    format!("{:02}:{:02}-{:02}:{:02}", sh, sm, eh, em)
}

fn format_day_slots(day: &crate::config::DaySchedule) -> String {
    let mut parts = Vec::with_capacity(2);
    if day.slot1().is_some() {
        parts.push(format_slot(day.start_hour, day.start_minute, day.end_hour, day.end_minute));
    }
    if day.slot2().is_some() {
        parts.push(format_slot(day.start2_hour, day.start2_minute, day.end2_hour, day.end2_minute));
    }
    if parts.is_empty() {
        lang::SLOTS_NONE.to_string()
    } else {
        parts.join(", ")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusType {
    AllowedNormal,
    AllowedTemporary,
    AllowedManual,
    BlockedSchedule,
    BlockedManual,
}

#[derive(Clone, Debug)]
pub struct StatusDetail {
    pub is_blocked: bool,
    #[allow(dead_code)]
    pub status_type: StatusType,
    pub summary: String,
    pub tooltip: String,
}

pub struct AppState {
    pub config: AppConfig,
    pub temp_allow_until: Option<DateTime<Local>>,
    pub manual_override: Option<bool>,
    pub last_applied_blocked: Option<bool>,
    last_apply_attempt: Option<Instant>,
    apply_failed: bool,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            temp_allow_until: None,
            manual_override: None,
            last_applied_blocked: None,
            last_apply_attempt: None,
            apply_failed: false,
        }
    }

    /// Sets temporary allow duration from now (e.g. 30 minutes, 60 minutes).
    pub fn set_temporary_allow(&mut self, minutes: i64) {
        let until = Local::now() + chrono::Duration::minutes(minutes);
        self.temp_allow_until = Some(until);
        self.manual_override = None; // clear manual override when temporary allow is set
    }

    /// Cancels temporary allow.
    pub fn cancel_temporary_allow(&mut self) {
        self.temp_allow_until = None;
    }

    /// Toggles manual override.
    pub fn toggle_manual_override(&mut self) {
        let currently_blocked = self.last_applied_blocked.unwrap_or(false);
        self.manual_override = Some(!currently_blocked);
        self.temp_allow_until = None;
    }

    /// Clears manual override, returning to schedule.
    pub fn clear_manual_override(&mut self) {
        self.manual_override = None;
    }

    fn weekday_index(now: DateTime<Local>) -> usize {
        now.weekday().num_days_from_monday() as usize
    }

    fn minutes_of_day(now: DateTime<Local>) -> u32 {
        now.hour() * 60 + now.minute()
    }

    fn block_source_at(&self, now: DateTime<Local>) -> Option<BlockSource> {
        let idx = Self::weekday_index(now);
        let cur = Self::minutes_of_day(now);
        let today = &self.config.weekly_schedule[idx % 7];

        for (start, end, slot) in today.active_slots() {
            if start < end {
                if cur >= start && cur < end {
                    return Some(BlockSource::Today { index: idx % 7, slot });
                }
            } else if cur >= start {
                // Crossing window: only the late part belongs to today.
                // The early-morning part is handled as yesterday's spillover.
                return Some(BlockSource::TodayLate { index: idx % 7, slot });
            }
        }

        // A crossing window continues into the next morning even when the
        // current day itself is unchecked.
        let yesterday = (idx + 6) % 7;
        let prev = &self.config.weekly_schedule[yesterday];
        for (start, end, slot) in prev.active_slots() {
            if start > end && cur < end {
                return Some(BlockSource::YesterdaySpillover { yesterday, slot });
            }
        }

        None
    }

    /// Checks if current local time falls into the block schedule window.
    pub fn is_time_in_schedule(&self, now: DateTime<Local>) -> bool {
        self.block_source_at(now).is_some()
    }

    /// End of the currently active block (hour, minute, source day index, slot).
    fn current_block_end(&self, now: DateTime<Local>) -> Option<(u32, u32, usize, usize)> {
        match self.block_source_at(now)? {
            BlockSource::Today { index, slot } | BlockSource::TodayLate { index, slot } => {
                let day = &self.config.weekly_schedule[index];
                let (eh, em) = day.slot_end_hm(slot);
                Some((eh, em, index, slot))
            }
            BlockSource::YesterdaySpillover { yesterday, slot } => {
                let day = &self.config.weekly_schedule[yesterday];
                let (eh, em) = day.slot_end_hm(slot);
                Some((eh, em, yesterday, slot))
            }
        }
    }

    /// Next block start within 7 days: (day index, hour, minute, days ahead, slot).
    fn next_block_start(&self, now: DateTime<Local>) -> Option<(usize, u32, u32, usize, usize)> {
        let idx = Self::weekday_index(now);
        let cur = Self::minutes_of_day(now);
        for days_ahead in 0..8 {
            let day_idx = (idx + days_ahead) % 7;
            let day = &self.config.weekly_schedule[day_idx];
            let mut candidates: Vec<(u32, usize)> = Vec::with_capacity(2);
            for (start, _end, slot) in day.active_slots() {
                if days_ahead == 0 {
                    // Currently allowed, so only a later start today counts.
                    if cur < start {
                        candidates.push((start, slot));
                    }
                } else {
                    candidates.push((start, slot));
                }
            }
            if candidates.is_empty() {
                continue;
            }
            candidates.sort();
            let (start, slot) = candidates[0];
            let (sh, sm) = day.slot_start_hm(slot);
            debug_assert_eq!(start, sh * 60 + sm);
            return Some((day_idx, sh, sm, days_ahead, slot));
        }
        None
    }

    fn describe_schedule(&self, now: DateTime<Local>) -> (String, String) {
        let idx = Self::weekday_index(now);
        if let Some((end_h, end_m, src_idx, slot)) = self.current_block_end(now) {
            let src = &self.config.weekly_schedule[src_idx];
            let src_name = WEEKDAY_SHORT[src_idx];
            let (sh, sm) = src.slot_start_hm(slot);
            let active = format_slot(sh, sm, end_h, end_m);
            let end = format!("{:02}:{:02}", end_h, end_m);
            let summary = lang::blocked_summary(src_name, &active, &format_day_slots(src), &end);
            let tooltip = lang::blocked_tooltip(src_name, &format_day_slots(src), &end);
            return (summary, tooltip);
        }

        match self.next_block_start(now) {
            Some((next_idx, _sh, _sm, _days_ahead, _slot)) => {
                let next = &self.config.weekly_schedule[next_idx];
                let today = &self.config.weekly_schedule[idx];
                let today_name = WEEKDAY_FULL[idx];
                let next_name = WEEKDAY_SHORT[next_idx];
                let summary = lang::allowed_summary(
                    today_name,
                    &format_day_slots(today),
                    next_name,
                    &format_day_slots(next),
                );
                let tooltip = lang::allowed_tooltip(today_name, next_name, &format_day_slots(next));
                (summary, tooltip)
            }
            None => (
                "Internet allowed (no weekday block scheduled)".to_string(),
                "Kid Internet Lock\nStatus: 🟢 Allowed\nNo weekday block scheduled".to_string(),
            ),
        }
    }

    /// Evaluates desired state and synchronizes Windows Firewall if state changed.
    pub fn evaluate_and_sync(&mut self) -> StatusDetail {
        let now = Local::now();

        // Expire temporary allow if time has passed
        if let Some(until) = self.temp_allow_until {
            if now >= until {
                self.temp_allow_until = None;
            }
        }

        let (should_block, status_type, mut summary, mut tooltip) = if let Some(forced_block) = self.manual_override {
            if forced_block {
                (
                    true,
                    StatusType::BlockedManual,
                    lang::MANUAL_BLOCKED_SUMMARY.to_string(),
                    lang::MANUAL_BLOCKED_TOOLTIP.to_string(),
                )
            } else {
                (
                    false,
                    StatusType::AllowedManual,
                    lang::MANUAL_ALLOWED_SUMMARY.to_string(),
                    lang::MANUAL_ALLOWED_TOOLTIP.to_string(),
                )
            }
        } else if let Some(until) = self.temp_allow_until {
            let rem_secs = (until - now).num_seconds();
            let rem_mins = (rem_secs + 59) / 60;
            (
                false,
                StatusType::AllowedTemporary,
                lang::temp_allow_summary(rem_mins),
                lang::temp_allow_tooltip(rem_mins, &until.format("%H:%M").to_string()),
            )
        } else {
            let in_schedule = self.is_time_in_schedule(now);
            let (summary, tooltip) = self.describe_schedule(now);
            if in_schedule {
                (true, StatusType::BlockedSchedule, summary, tooltip)
            } else {
                (false, StatusType::AllowedNormal, summary, tooltip)
            }
        };

        // Sync with firewall if desired state differs from last applied.
        // On failure the state is not recorded, so it is retried periodically.
        if self.last_applied_blocked != Some(should_block) {
            let retry_ready = self
                .last_apply_attempt
                .map(|attempt| attempt.elapsed() >= Duration::from_secs(15))
                .unwrap_or(true);

            if retry_ready {
                let result = if should_block {
                    FirewallManager::block_internet()
                } else {
                    FirewallManager::unblock_internet()
                };
                self.last_apply_attempt = Some(Instant::now());

                match result {
                    Ok(()) => {
                        self.last_applied_blocked = Some(should_block);
                        self.apply_failed = false;
                    }
                    Err(_) => {
                        self.apply_failed = true;
                    }
                }
            }
        } else {
            self.apply_failed = false;
        }

        if self.apply_failed {
            summary.push_str(lang::FIREWALL_FAIL_SUMMARY_SUFFIX);
            tooltip.push_str(lang::FIREWALL_FAIL_TOOLTIP_SUFFIX);
        }

        StatusDetail {
            is_blocked: should_block,
            status_type,
            summary,
            tooltip,
        }
    }
}

pub type SharedAppState = Arc<Mutex<AppState>>;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn cfg_with_same_window(sh: u32, sm: u32, eh: u32, em: u32) -> AppConfig {
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = true;
            day.start_hour = sh;
            day.start_minute = sm;
            day.end_hour = eh;
            day.end_minute = em;
            day.start2_hour = 0;
            day.start2_minute = 0;
            day.end2_hour = 0;
            day.end2_minute = 0;
        }
        cfg
    }

    #[test]
    fn test_standard_schedule() {
        let state = AppState::new(cfg_with_same_window(0, 0, 7, 0));

        let dt_block1 = Local.with_ymd_and_hms(2026, 9, 16, 0, 0, 0).unwrap();
        let dt_block2 = Local.with_ymd_and_hms(2026, 9, 16, 3, 30, 0).unwrap();
        let dt_block3 = Local.with_ymd_and_hms(2026, 9, 16, 6, 59, 0).unwrap();
        let dt_allow1 = Local.with_ymd_and_hms(2026, 9, 16, 7, 0, 0).unwrap();
        let dt_allow2 = Local.with_ymd_and_hms(2026, 9, 16, 15, 0, 0).unwrap();
        let dt_allow3 = Local.with_ymd_and_hms(2026, 9, 16, 23, 59, 0).unwrap();

        assert!(state.is_time_in_schedule(dt_block1));
        assert!(state.is_time_in_schedule(dt_block2));
        assert!(state.is_time_in_schedule(dt_block3));
        assert!(!state.is_time_in_schedule(dt_allow1));
        assert!(!state.is_time_in_schedule(dt_allow2));
        assert!(!state.is_time_in_schedule(dt_allow3));
    }

    #[test]
    fn test_midnight_crossing_schedule() {
        let state = AppState::new(cfg_with_same_window(23, 30, 6, 30));

        let dt_allow1 = Local.with_ymd_and_hms(2026, 9, 16, 23, 29, 0).unwrap();
        let dt_block1 = Local.with_ymd_and_hms(2026, 9, 16, 23, 30, 0).unwrap();
        // Early morning is covered by the previous day's crossing window.
        let dt_block2 = Local.with_ymd_and_hms(2026, 9, 16, 0, 0, 0).unwrap();
        let dt_block3 = Local.with_ymd_and_hms(2026, 9, 16, 6, 29, 0).unwrap();
        let dt_allow2 = Local.with_ymd_and_hms(2026, 9, 16, 6, 30, 0).unwrap();
        let dt_allow3 = Local.with_ymd_and_hms(2026, 9, 16, 12, 0, 0).unwrap();

        assert!(!state.is_time_in_schedule(dt_allow1));
        assert!(state.is_time_in_schedule(dt_block1));
        assert!(state.is_time_in_schedule(dt_block2));
        assert!(state.is_time_in_schedule(dt_block3));
        assert!(!state.is_time_in_schedule(dt_allow2));
        assert!(!state.is_time_in_schedule(dt_allow3));
    }

    #[test]
    fn test_weekday_specific_block() {
        // 2026-09-14 = Monday, 2026-09-15 = Tuesday.
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
        // Monday only: 00:00-07:00.
        cfg.weekly_schedule[0].enabled = true;
        cfg.weekly_schedule[0].start_hour = 0;
        cfg.weekly_schedule[0].start_minute = 0;
        cfg.weekly_schedule[0].end_hour = 7;
        cfg.weekly_schedule[0].end_minute = 0;

        let state = AppState::new(cfg);
        let mon_block = Local.with_ymd_and_hms(2026, 9, 14, 3, 0, 0).unwrap();
        let tue_allow = Local.with_ymd_and_hms(2026, 9, 15, 3, 0, 0).unwrap();
        assert!(state.is_time_in_schedule(mon_block));
        assert!(!state.is_time_in_schedule(tue_allow));
    }

    #[test]
    fn test_crossing_spillover_into_unchecked_day() {
        // Monday night 23:00-07:00 spills into Tuesday morning even if Tuesday is off.
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
        cfg.weekly_schedule[0].enabled = true;
        cfg.weekly_schedule[0].start_hour = 23;
        cfg.weekly_schedule[0].start_minute = 0;
        cfg.weekly_schedule[0].end_hour = 7;
        cfg.weekly_schedule[0].end_minute = 0;

        let state = AppState::new(cfg);
        let mon_late = Local.with_ymd_and_hms(2026, 9, 14, 23, 30, 0).unwrap();
        let tue_early = Local.with_ymd_and_hms(2026, 9, 15, 3, 0, 0).unwrap();
        let tue_late = Local.with_ymd_and_hms(2026, 9, 15, 8, 0, 0).unwrap();
        // Monday morning is not covered by Monday's own night window.
        let mon_early = Local.with_ymd_and_hms(2026, 9, 14, 3, 0, 0).unwrap();
        assert!(state.is_time_in_schedule(mon_late));
        assert!(state.is_time_in_schedule(tue_early));
        assert!(!state.is_time_in_schedule(tue_late));
        assert!(!state.is_time_in_schedule(mon_early));
    }

    #[test]
    fn test_all_days_disabled_allows_always() {
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
        let state = AppState::new(cfg);
        let dt = Local.with_ymd_and_hms(2026, 9, 16, 3, 0, 0).unwrap();
        assert!(!state.is_time_in_schedule(dt));
        assert!(state.next_block_start(dt).is_none());
    }

    #[test]
    fn test_second_slot_block() {
        // Monday: slot1 00:00-07:00, slot2 22:00-23:00.
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
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

        let state = AppState::new(cfg);
        let morning = Local.with_ymd_and_hms(2026, 9, 14, 3, 0, 0).unwrap();
        let evening = Local.with_ymd_and_hms(2026, 9, 14, 22, 30, 0).unwrap();
        let midday = Local.with_ymd_and_hms(2026, 9, 14, 12, 0, 0).unwrap();
        assert!(state.is_time_in_schedule(morning));
        assert!(state.is_time_in_schedule(evening));
        assert!(!state.is_time_in_schedule(midday));

        // Between the slots the next block is the evening slot today.
        let between = Local.with_ymd_and_hms(2026, 9, 14, 8, 0, 0).unwrap();
        let next = state.next_block_start(between).unwrap();
        assert_eq!((next.0, next.1, next.2, next.3), (0, 22, 0, 0));
    }

    #[test]
    fn test_second_slot_crossing_spillover() {
        // Monday slot2 23:00-01:00 spills into Tuesday morning.
        let mut cfg = AppConfig::default();
        for day in cfg.weekly_schedule.iter_mut() {
            day.enabled = false;
        }
        let mon = &mut cfg.weekly_schedule[0];
        mon.enabled = true;
        mon.start_hour = 0;
        mon.start_minute = 0;
        mon.end_hour = 0;
        mon.end_minute = 0;
        mon.start2_hour = 23;
        mon.start2_minute = 0;
        mon.end2_hour = 1;
        mon.end2_minute = 0;

        let state = AppState::new(cfg);
        let mon_late = Local.with_ymd_and_hms(2026, 9, 14, 23, 30, 0).unwrap();
        let tue_early = Local.with_ymd_and_hms(2026, 9, 15, 0, 30, 0).unwrap();
        let tue_late = Local.with_ymd_and_hms(2026, 9, 15, 2, 0, 0).unwrap();
        assert!(state.is_time_in_schedule(mon_late));
        assert!(state.is_time_in_schedule(tue_early));
        assert!(!state.is_time_in_schedule(tue_late));
    }

    #[test]
    fn test_temp_allow_and_manual_override() {
        let cfg = AppConfig::default();
        let mut state = AppState::new(cfg);

        // Initially no override
        assert!(state.temp_allow_until.is_none());
        assert!(state.manual_override.is_none());

        // Set 30 min allow
        state.set_temporary_allow(30);
        assert!(state.temp_allow_until.is_some());

        // Cancel
        state.cancel_temporary_allow();
        assert!(state.temp_allow_until.is_none());

        // Toggle manual override
        state.toggle_manual_override();
        assert_eq!(state.manual_override, Some(true));

        state.toggle_manual_override();
        assert_eq!(state.manual_override, Some(true)); // last_applied_blocked was None so !false = true

        state.clear_manual_override();
        assert!(state.manual_override.is_none());
    }
}
