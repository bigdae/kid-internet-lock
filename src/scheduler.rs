use chrono::{DateTime, Local, Timelike};
use crate::config::AppConfig;
use crate::firewall::FirewallManager;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

    /// Checks if current local time falls into the block schedule window.
    pub fn is_time_in_schedule(&self, now: DateTime<Local>) -> bool {
        let cur = now.hour() * 60 + now.minute();
        let start = self.config.start_hour * 60 + self.config.start_minute;
        let end = self.config.end_hour * 60 + self.config.end_minute;

        if start == end {
            // Disabled when start equals end
            return false;
        }

        if start < end {
            // Same day: e.g. 01:00 to 07:00
            cur >= start && cur < end
        } else {
            // Midnight crossing: e.g. 23:00 to 07:00
            cur >= start || cur < end
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
                    "Internet manually blocked".to_string(),
                    "Kid Internet Lock\nStatus: 🔴 Manually Blocked\n(Manual control active)".to_string(),
                )
            } else {
                (
                    false,
                    StatusType::AllowedManual,
                    "Internet manually allowed".to_string(),
                    "Kid Internet Lock\nStatus: 🟢 Manually Allowed\n(Manual control active)".to_string(),
                )
            }
        } else if let Some(until) = self.temp_allow_until {
            let rem_secs = (until - now).num_seconds();
            let rem_mins = (rem_secs + 59) / 60;
            (
                false,
                StatusType::AllowedTemporary,
                format!("Temporary allow active (about {} min left)", rem_mins),
                format!(
                    "Kid Internet Lock\nStatus: 🟢 Temporary allow active\nRemaining: about {} min (until {})",
                    rem_mins,
                    until.format("%H:%M")
                ),
            )
        } else {
            let in_schedule = self.is_time_in_schedule(now);
            if in_schedule {
                (
                    true,
                    StatusType::BlockedSchedule,
                    format!("Internet blocked for tonight (unblocks at {:02}:{:02})", self.config.end_hour, self.config.end_minute),
                    format!(
                        "Kid Internet Lock\nStatus: 🔴 Nightly internet block active\nUnblocks at: {:02}:{:02}",
                        self.config.end_hour, self.config.end_minute
                    ),
                )
            } else {
                (
                    false,
                    StatusType::AllowedNormal,
                    format!("Internet allowed (block starts at {:02}:{:02})", self.config.start_hour, self.config.start_minute),
                    format!(
                        "Kid Internet Lock\nStatus: 🟢 Internet allowed\nBlock starts at: {:02}:{:02}",
                        self.config.start_hour, self.config.start_minute
                    ),
                )
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
            summary.push_str(" ⚠️ Failed to apply firewall rule");
            tooltip.push_str("\n⚠️ Failed to apply the firewall rule (check security policy/permissions)");
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

    #[test]
    fn test_standard_schedule() {
        let mut cfg = AppConfig::default();
        cfg.start_hour = 0;
        cfg.start_minute = 0;
        cfg.end_hour = 7;
        cfg.end_minute = 0;

        let state = AppState::new(cfg);

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
        let mut cfg = AppConfig::default();
        cfg.start_hour = 23;
        cfg.start_minute = 30;
        cfg.end_hour = 6;
        cfg.end_minute = 30;

        let state = AppState::new(cfg);

        let dt_allow1 = Local.with_ymd_and_hms(2026, 9, 16, 23, 29, 0).unwrap();
        let dt_block1 = Local.with_ymd_and_hms(2026, 9, 16, 23, 30, 0).unwrap();
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
