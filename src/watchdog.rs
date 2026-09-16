use std::mem::{size_of, zeroed};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateProcessW, GetCurrentProcessId, OpenProcess, ResetEvent, SetEvent,
    WaitForMultipleObjects, WaitForSingleObject, CREATE_NO_WINDOW, INFINITE,
    PROCESS_INFORMATION, PROCESS_SYNCHRONIZE, STARTUPINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SHUTTINGDOWN};

const SHUTDOWN_EVENT_NAME: &str = "KidInternetLock_Shutdown_Event";
const GUARD_TASK_NAME: &str = "KidInternetLock_Guard";

static SHUTDOWN_EVENT: AtomicIsize = AtomicIsize::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn exe_path_string() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

unsafe fn spawn_process(command_line: &str) -> Option<HANDLE> {
    unsafe {
        let mut cmd = wide(command_line);
        let mut si: STARTUPINFOW = zeroed();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        let mut pi: PROCESS_INFORMATION = zeroed();

        let ok = CreateProcessW(
            null_mut(),
            cmd.as_mut_ptr(),
            null_mut(),
            null_mut(),
            0,
            CREATE_NO_WINDOW,
            null_mut(),
            null_mut(),
            &si,
            &mut pi,
        );

        if ok == 0 {
            return None;
        }

        CloseHandle(pi.hThread);
        Some(pi.hProcess)
    }
}

unsafe fn shutdown_signaled(event: HANDLE) -> bool {
    unsafe { !event.is_null() && WaitForSingleObject(event, 0) == WAIT_OBJECT_0 }
}

unsafe fn system_is_shutting_down() -> bool {
    unsafe { GetSystemMetrics(SM_SHUTTINGDOWN) != 0 }
}

unsafe fn run_schtasks(args: &str) {
    unsafe {
        let system_root =
            std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let command_line = format!("\"{}\\System32\\schtasks.exe\" {}", system_root, args);
        if let Some(handle) = spawn_process(&command_line) {
            WaitForSingleObject(handle, 15000);
            CloseHandle(handle);
        }
    }
}

/// Registers a scheduled task that relaunches the app every minute when it is
/// not running. This is what revives the app even if every process is killed
/// at once from Task Manager. XML registration is used so the task also runs
/// on battery power and starts as soon as possible after logon.
fn ensure_guard_task() {
    let Some(exe) = exe_path_string() else {
        return;
    };

    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(name)) if !domain.is_empty() && !name.is_empty() => {
            format!("{}\\{}", domain, name)
        }
        (_, Ok(name)) if !name.is_empty() => name,
        _ => return,
    };

    let now = chrono::Local::now();
    let mut triggers = String::new();
    for step in 0..6 {
        let boundary = (now + chrono::Duration::seconds(step * 10))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        triggers.push_str(&format!(
            "    <TimeTrigger>\n\
      <StartBoundary>{boundary}</StartBoundary>\n\
      <Repetition>\n\
        <Interval>PT1M</Interval>\n\
        <StopAtDurationEnd>false</StopAtDurationEnd>\n\
      </Repetition>\n\
      <Enabled>true</Enabled>\n\
    </TimeTrigger>\n"
        ));
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n\
<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n\
  <RegistrationInfo>\n\
    <Description>KidInternetLock keep-alive</Description>\n\
  </RegistrationInfo>\n\
  <Principals>\n\
    <Principal id=\"Author\">\n\
      <UserId>{user}</UserId>\n\
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
{triggers}  </Triggers>\n\
  <Actions Context=\"Author\">\n\
    <Exec>\n\
      <Command>\"{exe}\"</Command>\n\
      <Arguments>--silent</Arguments>\n\
    </Exec>\n\
  </Actions>\n\
</Task>\n",
        user = xml_escape(&user),
        triggers = triggers,
        exe = xml_escape(&exe),
    );

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let xml_path = std::env::temp_dir().join(format!(
        "kid_internet_lock_guard_{}_{}.xml",
        std::process::id(),
        nonce
    ));
    let mut bytes = Vec::with_capacity(xml.len() * 2 + 2);
    bytes.push(0xFF);
    bytes.push(0xFE);
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    if std::fs::write(&xml_path, &bytes).is_err() {
        return;
    }

    let args = format!(
        "/Create /F /TN \"{}\" /XML \"{}\"",
        GUARD_TASK_NAME,
        xml_path.to_string_lossy()
    );
    unsafe { run_schtasks(&args) };

    let _ = std::fs::remove_file(&xml_path);
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn remove_guard_task() {
    let args = format!("/Delete /F /TN \"{}\"", GUARD_TASK_NAME);
    unsafe { run_schtasks(&args) };
}

/// Starts the mutual watchdog: spawns a hidden helper process that relaunches
/// this app if it is terminated, and keeps that helper alive while running.
pub fn start_guardian() {
    unsafe {
        let event = CreateEventW(null_mut(), 1, 0, wide(SHUTDOWN_EVENT_NAME).as_ptr());
        if event.is_null() {
            return;
        }
        ResetEvent(event);
        SHUTDOWN_EVENT.store(event as isize, Ordering::SeqCst);

        let event_addr = event as isize;
        let main_pid = GetCurrentProcessId();
        std::thread::spawn(move || monitor_watchdog(event_addr as HANDLE, main_pid));
    }
}

/// Signals the watchdog that the app is exiting on purpose.
pub fn signal_shutdown() {
    let handle = SHUTDOWN_EVENT.load(Ordering::SeqCst);
    if handle != 0 {
        unsafe {
            SetEvent(handle as HANDLE);
        }
    }
}

/// Intentional exit: stop the watchdog and remove the auto-restart layers.
pub fn stop_guardian() {
    signal_shutdown();
    remove_guard_task();
    crate::service::stop_and_remove();
}

fn monitor_watchdog(event: HANDLE, main_pid: u32) {
    let Some(exe) = exe_path_string() else {
        return;
    };

    if !unsafe { shutdown_signaled(event) } {
        ensure_guard_task();
        if unsafe { shutdown_signaled(event) } {
            remove_guard_task();
            return;
        }
    }

    let command = format!("\"{}\" --watchdog {}", exe, main_pid);

    loop {
        let started = Instant::now();
        match unsafe { spawn_process(&command) } {
            Some(handle) => unsafe {
                WaitForSingleObject(handle, INFINITE);
                CloseHandle(handle);
            },
            None => std::thread::sleep(Duration::from_secs(10)),
        }

        if unsafe { shutdown_signaled(event) || system_is_shutting_down() } {
            break;
        }

        if started.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_secs(5));
        } else {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

/// Runs inside the hidden helper process. Waits for the main process to
/// disappear and relaunches it unless the exit was intentional or the system
/// is shutting down.
pub fn run_watchdog(target_pid: u32) -> i32 {
    unsafe {
        let event = CreateEventW(null_mut(), 1, 0, wide(SHUTDOWN_EVENT_NAME).as_ptr());
        if event.is_null() {
            return 0;
        }

        let target = OpenProcess(PROCESS_SYNCHRONIZE, 0, target_pid);
        let wait_started = Instant::now();
        if !target.is_null() {
            let handles = [target, event];
            WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE);
            CloseHandle(target);
        }
        let target_lifetime = wait_started.elapsed();

        if shutdown_signaled(event) || system_is_shutting_down() {
            CloseHandle(event);
            return 0;
        }

        let delay = if target_lifetime < Duration::from_secs(5) {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(2)
        };
        std::thread::sleep(delay);

        if shutdown_signaled(event) || system_is_shutting_down() {
            CloseHandle(event);
            return 0;
        }

        if let Some(exe) = exe_path_string() {
            if let Some(handle) = spawn_process(&format!("\"{}\" --silent", exe)) {
                CloseHandle(handle);
            }
        }

        CloseHandle(event);
        0
    }
}
