@echo off
chcp 65001 > nul
cd /d "%~dp0"

if not exist "%~dp0KidInternetLock.exe" (
    if exist "%~dp0target\release\kid_internet_lock.exe" (
        copy /Y "%~dp0target\release\kid_internet_lock.exe" "%~dp0KidInternetLock.exe" > nul
    ) else (
        echo [Notice] Executable not found. Building...
        call "%~dp0build.bat"
    )
)

echo [Kid Internet Lock] Starting...
schtasks /query /tn "KidInternetLock_AutoStart" >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    schtasks /run /tn "KidInternetLock_AutoStart" >nul 2>&1
) else (
    start "" "%~dp0KidInternetLock.exe"
)
