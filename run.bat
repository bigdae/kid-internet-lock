@echo off
chcp 65001 > nul
cd /d "%~dp0"

if not exist "%~dp0KidInternetLock.exe" (
    if exist "%~dp0target\release\kid_internet_lock.exe" (
        copy /Y "%~dp0target\release\kid_internet_lock.exe" "%~dp0KidInternetLock.exe" > nul
    ) else (
        echo [알림] 실행 파일이 없어 빌드를 진행합니다...
        call "%~dp0build.bat"
    )
)

echo [Kid Internet Lock] 야간 인터넷 지킴이를 실행합니다...
start "" "%~dp0KidInternetLock.exe"
