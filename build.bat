@echo off
chcp 65001 > nul
echo [Kid Internet Lock] Starting Rust release build...
cargo build --release
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

echo [Code Signing] Applying local digital signature...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0sign_app.ps1"
echo.
pause
