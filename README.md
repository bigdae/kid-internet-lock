# 🌙 Kid Internet Lock

> A tiny Windows program that blocks the internet automatically when it's bedtime.
> 100% Rust, single file (~0.6 MB), no install needed.

## How it works

`KidInternetLock.exe` lives in the system tray and toggles Windows Firewall rules on a per-weekday schedule. When a block window starts, the internet turns itself off; when the window ends, it turns back on. A read-only viewer (`InternetSchedule.exe`) shows kids today's usable hours at a glance.

## Features

- 📅 **Per-weekday block schedule (2 slots per day)** — different hours for each day, midnight crossing supported (e.g. `23:00–07:00` continues into the next morning)
- ⚡ **Batch input** — type a slot once, copy it to every weekday
- 👀 **Today-first kid viewer** — big "Today" headline plus a 24-hour clock timetable (green = available, red = blocked, yellow marker = now), today's ranges, tomorrow's preview, and a compact weekly list
- 🟢🔴 **Tray icon** — color shows the current state, tooltip shows the next block/release time
- 🔑 **Admin password** — settings, temporary access, manual override, and exit all require it (default `1q2w3e`, stored as a salted SHA-256 hash)
- ⏱️ **Temporary allow (30 min / 1 hour)** — for homework emergencies, plus a manual block/unblock toggle
- 🛡️ **Tamper-resistant** — a guard service + watchdog bring the app back if it is killed from Task Manager; single-instance lock
- 🚀 **UAC-free autostart** — optional logon task (Task Scheduler, highest privileges), so there is no UAC prompt at boot
- 🖼️ **Schedule image export** — save the weekly timetable as a BMP on the Desktop
- 🌐 **English/Korean UI** — `build.bat` picks the language from the Windows locale automatically

## Usage

1. Run `KidInternetLock.exe` → approve the UAC prompt (admin rights are needed for the firewall)
2. Right-click the tray icon → **Admin Settings** (password `1q2w3e`) → set times per weekday → **Save Settings**
3. Put an `InternetSchedule.exe` shortcut on the kid's desktop — done

## The kid viewer (`InternetSchedule.exe`)

Read-only, needs no admin rights. From top to bottom:

1. Current status line — available now (until when?) or blocked now (available from when?)
2. Big today headline (`Today, Thursday Sep 18`) — no weekday hunting
3. 24-hour timetable bar with 3-hour ticks and a yellow "now" marker
4. `Available today: 07:00 - 22:00` in large type
5. `Tomorrow (Friday): ...` preview
6. Compact weekly list — today's row is bold and marked with `>`

It refreshes every 30 seconds.

## Schedule rules

- Unchecking a day = allowed all day
- A slot with start == end (e.g. `00:00 - 00:00`) is unused
- A slot crossing midnight (start > end) continues into the next morning, even if the next day is unchecked

## Build from source

From an elevated terminal:

```
build.bat
```

This auto-detects the Windows locale (Korean Windows → Korean UI), builds both binaries, copies them to `KidInternetLock.exe` / `InternetSchedule.exe`, and applies a local digital signature.

Force English: `cargo build --release`
Force Korean: `cargo build --release --features ko`

Run tests: `cargo test` (or `cargo test --features ko`)

## Configuration & files

- `config.json` lives next to the exe when present, otherwise in `%APPDATA%\KidInternetLock\config.json`. Old configs without a weekly schedule are migrated automatically.
- Autostart uses the Task Scheduler task `KidInternetLock_AutoStart` (logon trigger, highest privileges).
- Built executables (`*.exe`) and `/target` are gitignored — releases distribute the built files.

## Good to know

- A video already playing may continue briefly while blocked — restart the app and it applies immediately
- Exiting needs the password too. A normal exit restores the firewall rules cleanly
