@echo off
chcp 65001 > nul
echo [Kid Internet Lock] Starting Rust release build...
REM Auto-select UI language from Windows locale: Korean Windows -> Korean UI.
set WINLANG=
for /f "delims=" %%L in ('powershell -NoProfile -Command "(Get-Culture).Name"') do set WINLANG=%%L
echo [Language] Windows locale: %WINLANG%
echo %WINLANG% | findstr /i /b "ko" > nul
if %ERRORLEVEL% EQU 0 (
    echo [Language] Korean Windows detected - building Korean UI...
    cargo build --release --features ko
) else (
    echo [Language] Building default English UI...
    cargo build --release
)
if %ERRORLEVEL% NEQ 0 (
    echo [Error] Build failed.
    pause
    exit /b %ERRORLEVEL%
)

copy /Y "%~dp0target\release\kid_internet_lock.exe" "%~dp0KidInternetLock.exe" > nul
echo [Success] Standalone executable created: KidInternetLock.exe
echo File size:
dir /-C "%~dp0KidInternetLock.exe" | findstr /i "KidInternetLock.exe"
echo.
copy /Y "%~dp0target\release\schedule_viewer.exe" "%~dp0InternetSchedule.exe" > nul
echo [Success] Kid schedule viewer created: InternetSchedule.exe
echo.

echo [Code Signing] Applying local digital signature...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0sign_app.ps1"
echo.
pause
