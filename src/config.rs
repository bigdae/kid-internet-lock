use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
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

    /// Syncs Windows registry auto-start setting.
    pub fn sync_autostart_registry(&self) -> Result<(), String> {
        set_autostart_registry(self.auto_start)
    }
}

/// Sets or deletes the HKCU Run registry value.
pub fn set_autostart_registry(enable: bool) -> Result<(), String> {
    let key_wide: Vec<u16> = RUN_KEY_PATH.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = RUN_VALUE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut hkey = null_mut();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            key_wide.as_ptr(),
            0,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            &mut hkey,
        );

        if status != 0 {
            return Err(format!("Failed to open registry key: error code {}", status));
        }

        let result = if enable {
            let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
            let formatted_cmd = format!("\"{}\" --silent", exe_path.to_string_lossy());
            let cmd_wide: Vec<u16> = formatted_cmd.encode_utf16().chain(std::iter::once(0)).collect();

            let set_res = RegSetValueExW(
                hkey,
                val_wide.as_ptr(),
                0,
                REG_SZ,
                cmd_wide.as_ptr() as *const u8,
                (cmd_wide.len() * std::mem::size_of::<u16>()) as u32,
            );
            if set_res == 0 {
                Ok(())
            } else {
                Err(format!("Failed to set registry value: error code {}", set_res))
            }
        } else {
            let del_res = RegDeleteValueW(hkey, val_wide.as_ptr());
            // 2 is ERROR_FILE_NOT_FOUND, which means already deleted/not present.
            if del_res == 0 || del_res == 2 {
                Ok(())
            } else {
                Err(format!("Failed to delete registry value: error code {}", del_res))
            }
        };

        RegCloseKey(hkey);
        result
    }
}

/// Checks whether autostart is registered in HKCU Run registry.
#[allow(dead_code)]
pub fn is_autostart_registered() -> bool {
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
