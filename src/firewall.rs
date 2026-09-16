use std::os::windows::process::CommandExt;
use std::process::Command;

pub const FIREWALL_RULE_NAME: &str = "KidInternetLock_Outbound_Block";
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct FirewallManager;

impl FirewallManager {
    /// Enables Windows Firewall on every profile. Block rules have no effect
    /// on profiles whose firewall is turned off (common on public networks),
    /// so this must run before adding the block rule.
    fn ensure_firewall_enabled() -> Result<(), String> {
        let output = Command::new("netsh")
            .args(["advfirewall", "set", "allprofiles", "state", "on"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute netsh: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let err_msg = if !stderr.is_empty() { stderr } else { stdout };
            Err(format!(
                "Failed to enable Windows Firewall: {}",
                err_msg.trim()
            ))
        }
    }

    /// Blocks all outbound internet traffic by adding the Windows Firewall rule.
    pub fn block_internet() -> Result<(), String> {
        // Delete any existing rule first to prevent duplicates
        let _ = Self::unblock_internet();

        Self::ensure_firewall_enabled()?;

        let output = Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "add",
                "rule",
                &format!("name={}", FIREWALL_RULE_NAME),
                "dir=out",
                "action=block",
                "enable=yes",
                "profile=any",
                "description=Kid Internet Lock Automated Outbound Block Rule",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute netsh: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let err_msg = if !stderr.is_empty() {
                stderr.to_string()
            } else {
                stdout.to_string()
            };
            return Err(format!("Firewall block rule failed: {}", err_msg.trim()));
        }

        if Self::is_rule_active() {
            Ok(())
        } else {
            Err("Firewall block rule was not applied".to_string())
        }
    }

    /// Removes the outbound block rule, restoring internet access.
    pub fn unblock_internet() -> Result<(), String> {
        let output = Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "delete",
                "rule",
                &format!("name={}", FIREWALL_RULE_NAME),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute netsh: {}", e))?;

        // Note: if the rule wasn't present, netsh might exit with non-zero, which is acceptable
        let _ = output;
        Ok(())
    }

    /// Checks if the firewall block rule currently exists and is active.
    pub fn is_rule_active() -> bool {
        let output = Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "show",
                "rule",
                &format!("name={}", FIREWALL_RULE_NAME),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    text.contains(FIREWALL_RULE_NAME)
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
}
