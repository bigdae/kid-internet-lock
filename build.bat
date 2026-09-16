@echo off
chcp 65001 > nul
echo [Kid Internet Lock] Rust 릴리즈 빌드를 시작합니다...
cargo build --release
if %ERRORLEVEL% NEQ 0 (
    echo [오류] 빌드에 실패했습니다.
    pause
    exit /b %ERRORLEVEL%
)

copy /Y "%~dp0target\release\kid_internet_lock.exe" "%~dp0KidInternetLock.exe" > nul
echo [성공] 무설치 단일 실행 파일이 생성되었습니다: KidInternetLock.exe
echo 파일 크기:
dir /-C "%~dp0KidInternetLock.exe" | findstr /i "KidInternetLock.exe"
echo.
pause
