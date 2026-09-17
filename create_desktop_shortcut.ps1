# create_desktop_shortcut.ps1
# Creates a desktop shortcut that launches Kid Internet Lock without UAC prompts
# using Windows Task Scheduler.

$WshShell = New-Object -ComObject WScript.Shell
$DesktopPath = [System.Environment]::GetFolderPath('Desktop')
$ShortcutPath = Join-Path $DesktopPath "Kid Internet Lock.lnk"
$ExePath = Join-Path $PSScriptRoot "KidInternetLock.exe"

$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = "$env:SystemRoot\System32\schtasks.exe"
$Shortcut.Arguments = '/run /tn "KidInternetLock_AutoStart"'
if (Test-Path $ExePath) {
    $Shortcut.IconLocation = "$ExePath,0"
}
$Shortcut.Description = "Launch Kid Internet Lock without UAC prompt"
$Shortcut.Save()

Write-Host ">>> Created desktop shortcut: $ShortcutPath" -ForegroundColor Green
Write-Host "    Double-clicking this shortcut will launch Kid Internet Lock elevated without UAC prompts." -ForegroundColor Cyan
