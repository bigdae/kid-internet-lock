# 🌙 Kid Internet Lock

> A tiny Windows program that blocks the internet automatically when it's bedtime.
> 100% Rust, single file (~0.6 MB), no install needed.

---

## Why use it?

- Telling the kids to get off YouTube at midnight just doesn't work
- At the set time the internet **turns itself off**, and **back on** when the window ends
- Kids can check the schedule themselves with a simple read-only viewer

## Features

- 📅 **Per-weekday block times (2 slots a day)** — different hours for each day, midnight crossing supported (e.g. 23:00–07:00)
- ⚡ **Batch input** — type once, copy to every weekday
- 🟢🔴 **Lives in the tray** — icon color shows the state, tooltip shows the next block/release time
- 🔑 **Admin password** — settings, temporary access and exit all need it (default `1q2w3e`)
- ⏱️ **Temporary allow (30 min / 1 hour)** — for homework emergencies
- 🛡️ **Tamper-resistant** — a guard service + watchdog bring it back if killed from Task Manager
- 👶 **Kid schedule viewer** (`InternetSchedule.exe`) — read-only "is it on now, and when?" screen
- 🌐 **English/Korean UI** — `build.bat` picks the language from your Windows locale automatically

## Usage

1. Run `KidInternetLock.exe` → approve the UAC prompt (needs admin rights for the firewall)
2. Right-click the tray icon → **Admin Settings** (password `1q2w3e`) → set times per weekday → **Save Settings**
3. Put an `InternetSchedule.exe` shortcut on the kid's desktop — done

## Build from source

From an elevated terminal:

```
build.bat
```

Force English: `cargo build --release`
Force Korean: `cargo build --release --features ko`

## Good to know

- A video already playing may continue briefly while blocked — restart the app and it applies immediately
- Exiting needs the password too. A normal exit restores the firewall rules cleanly
