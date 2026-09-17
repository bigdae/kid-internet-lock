use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
};

const DEFAULT_SALT: &str = "kid_internet_lock_salt_v1";
const DEFAULT_PASSWORD: &str = "1q2w3e";
const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE_NAME: &str = "KidInternetLock";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub start_hour: u32,
    pub start_minute: u32,
    pub end_hour: u32,
    pub end_minute: u32,
    pub password_salt: String,
    pub password_hash: String,
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let salt = DEFAULT_SALT.to_string();
        let hash = hash_password(DEFAULT_PASSWORD, &salt);
        Self {
            start_hour: 0,
            start_minute: 0,
            end_hour: 7,
            end_minute: 0,
            password_salt: salt,
            password_hash: hash,
            auto_start: false,
        }
    }
}

pub fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_salt() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let mut hasher = Sha256::new();
    hasher.update(now.to_le_bytes());
    hasher.update(pid.to_le_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16])
}

impl AppConfig {
    pub fn verify_password(&self, input: &str) -> bool {
        let hashed = hash_password(input, &self.password_salt);
        hashed == self.password_hash
    }

    pub fn update_password(&mut self, new_password: &str) {
        let salt = generate_salt();
        let hash = hash_password(new_password, &salt);
        self.password_salt = salt;
        self.password_hash = hash;
    }

    /// Gets the path where config.json is stored.
    pub fn config_path() -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let local_config = exe_dir.join("config.json");
                if local_config.exists() {
                    return local_config;
                }
            }
        }

        if let Ok(appdata) = std::env::var("APPDATA") {
            let dir = Path::new(&appdata).join("KidInternetLock");
            let _ = fs::create_dir_all(&dir);
            return dir.join("config.json");
        }

        PathBuf::from("config.json")
    }

    /// Loads configuration from disk, creating default if not found.
    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(mut file) = File::open(&path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&contents) {
                    return cfg;
                }
            }
        }

        let default_cfg = Self::default();
        let _ = default_cfg.save();
        default_cfg
    }

    /// Saves configuration to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let mut file = File::create(&path).map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Syncs Windows auto-start setting using Task Scheduler (HighestAvailable)
    /// to run elevated at logon without UAC confirmation prompts.
    pub fn sync_autostart(&self) -> Result<(), String> {
        set_autostart(self.auto_start)
    }

    /// Backward compatibility alias for sync_autostart.
    #[allow(dead_code)]
    pub fn sync_autostart_registry(&self) -> Result<(), String> {
        self.sync_autostart()
    }
}

pub const AUTOSTART_TASK_NAME: &str = "KidInternetLock_AutoStart";
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn get_current_user_principal() -> Option<String> {
    match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(name)) if !domain.is_empty() && !name.is_empty() => {
            Some(format!("{}\\{}", domain, name))
        }
        (_, Ok(name)) if !name.is_empty() => Some(name),
        _ => None,
    }
}

/// Creates or updates the Task Scheduler task to run the app at Windows logon
/// with elevated administrator privileges (RunLevel: HighestAvailable).
/// This eliminates any UAC "Do you want to allow this app..." prompt at boot time.
pub fn create_autostart_task() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe_path.to_string_lossy().to_string();

    let user = get_current_user_principal().unwrap_or_default();
    let user_principal_xml = if user.is_empty() {
        String::new()
    } else {
        format!("      <UserId>{}</UserId>\n", xml_escape(&user))
    };
    let user_trigger_xml = if user.is_empty() {
        String::new()
    } else {
        format!("      <UserId>{}</UserId>\n", xml_escape(&user))
    };

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n\
<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n\
  <RegistrationInfo>\n\
    <Description>KidInternetLock startup at logon with highest privileges</Description>\n\
  </RegistrationInfo>\n\
  <Principals>\n\
    <Principal id=\"Author\">\n\
{user_principal_xml}\
      <LogonType>InteractiveToken</LogonType>\n\
      <RunLevel>HighestAvailable</RunLevel>\n\
    </Principal>\n\
  </Principals>\n\
  <Settings>\n\
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\n\
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\n\
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\n\
    <StartWhenAvailable>true</StartWhenAvailable>\n\
    <AllowStartOnDemand>true</AllowStartOnDemand>\n\
    <Enabled>true</Enabled>\n\
    <Hidden>false</Hidden>\n\
    <RunOnlyIfIdle>false</RunOnlyIfIdle>\n\
    <WakeToRun>false</WakeToRun>\n\
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>\n\
    <Priority>7</Priority>\n\
  </Settings>\n\
  <Triggers>\n\
    <LogonTrigger>\n\
      <Enabled>true</Enabled>\n\
{user_trigger_xml}\
    </LogonTrigger>\n\
  </Triggers>\n\
  <Actions Context=\"Author\">\n\
    <Exec>\n\
      <Command>\"{exe}\"</Command>\n\
      <Arguments>--silent</Arguments>\n\
    </Exec>\n\
  </Actions>\n\
</Task>\n",
        user_principal_xml = user_principal_xml,
        user_trigger_xml = user_trigger_xml,
        exe = xml_escape(&exe_str),
    );

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let xml_path = std::env::temp_dir().join(format!(
        "kid_internet_lock_autostart_{}_{}.xml",
        std::process::id(),
        nonce
    ));

    let mut bytes = Vec::with_capacity(xml.len() * 2 + 2);
    bytes.push(0xFF);
    bytes.push(0xFE);
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    fs::write(&xml_path, &bytes).map_err(|e| format!("Failed to write task XML: {}", e))?;

    let output = Command::new("schtasks")
        .args(&[
            "/Create",
            "/F",
            "/TN",
            AUTOSTART_TASK_NAME,
            "/XML",
            &xml_path.to_string_lossy(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let _ = fs::remove_file(&xml_path);

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!(
            "Failed to register autostart task: {}",
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Err(format!("Failed to execute schtasks: {}", e)),
    }
}

/// Deletes the autostart scheduled task.
pub fn delete_autostart_task() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let output = Command::new("schtasks")
        .args(&["/Delete", "/F", "/TN", AUTOSTART_TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to delete autostart task: {}", e)),
    }
}

/// Removes any legacy HKCU Run registry value if it was set in earlier versions.
pub fn remove_legacy_run_registry() -> Result<(), String> {
    let key_wide: Vec<u16> = RUN_KEY_PATH.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = RUN_VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey = null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_wide.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        ) != 0
        {
            return Ok(());
        }

        let _ = RegDeleteValueW(hkey, val_wide.as_ptr());
        RegCloseKey(hkey);
        Ok(())
    }
}

/// Sets or deletes the autostart configuration.
/// Uses Windows Task Scheduler with HighestAvailable privileges to prevent UAC prompts at logon,
/// and ensures legacy HKCU\Run registry entries are cleaned up.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    let _ = remove_legacy_run_registry();

    if enable {
        create_autostart_task()
    } else {
        delete_autostart_task()
    }
}

/// Backward compatibility function for existing callers.
#[allow(dead_code)]
pub fn set_autostart_registry(enable: bool) -> Result<(), String> {
    set_autostart(enable)
}

/// Checks whether autostart is registered in Windows Task Scheduler or legacy registry.
#[allow(dead_code)]
pub fn is_autostart_registered() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let output = Command::new("schtasks")
        .args(&["/Query", "/TN", AUTOSTART_TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            return true;
        }
    }

    // Check legacy registry as secondary fallback
    let key_wide: Vec<u16> = RUN_KEY_PATH.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = RUN_VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, key_wide.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
            return false;
        }

        let mut data_type = 0u32;
        let mut data_size = 0u32;
        let status = RegQueryValueExW(
            hkey,
            val_wide.as_ptr(),
            null_mut(),
            &mut data_type,
            null_mut(),
            &mut data_size,
        );

        RegCloseKey(hkey);
        status == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_password() {
        let cfg = AppConfig::default();
        assert!(cfg.verify_password("1q2w3e"));
        assert!(!cfg.verify_password("wrong_password"));
    }

    #[test]
    fn test_password_update() {
        let mut cfg = AppConfig::default();
        cfg.update_password("my_new_secret!123");
        assert!(!cfg.verify_password("1q2w3e"));
        assert!(cfg.verify_password("my_new_secret!123"));
    }
}
