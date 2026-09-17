# sign_app.ps1
# Creates a local trusted self-signed Code Signing Certificate and signs KidInternetLock.exe
# to eliminate "Unknown Publisher" / "알 수 없는 앱" warnings.

param(
    [string]$ExePath = "$PSScriptRoot\KidInternetLock.exe"
)

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  Kid Internet Lock - Code Signing Setup" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

if (-not (Test-Path $ExePath)) {
    Write-Host "[!] Executable not found at: $ExePath" -ForegroundColor Red
    exit 1
}

$certSubject = "CN=KidInternetLock"

# 1. Search for existing certificate in CurrentUser\My
$cert = Get-ChildItem -Path Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq $certSubject } | Select-Object -First 1

if (-not $cert) {
    Write-Host "[1/3] Creating self-signed Code Signing Certificate ($certSubject)..." -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $certSubject `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotAfter (Get-Date).AddYears(10)
    Write-Host "      Created certificate: $($cert.Thumbprint)" -ForegroundColor Green
} else {
    Write-Host "[1/3] Using existing certificate: $($cert.Thumbprint)" -ForegroundColor Green
}

# 2. Ensure public certificate is installed in Trusted Root Certification Authorities
$rootCert = Get-ChildItem -Path Cert:\CurrentUser\Root | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
if (-not $rootCert) {
    Write-Host "[2/3] Registering certificate in Trusted Root Certification Authorities..." -ForegroundColor Yellow
    $tempCer = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "KidInternetLock_$($cert.Thumbprint).cer")
    try {
        Export-Certificate -Cert $cert -FilePath $tempCer | Out-Null
        Import-Certificate -FilePath $tempCer -CertStoreLocation "Cert:\CurrentUser\Root" | Out-Null
        Write-Host "      Added to Cert:\CurrentUser\Root successfully." -ForegroundColor Green
    } finally {
        if (Test-Path $tempCer) {
            Remove-Item -Force $tempCer
        }
    }
} else {
    Write-Host "[2/3] Certificate is already trusted in Root store." -ForegroundColor Green
}

# 3. Sign the executable
Write-Host "[3/3] Signing $ExePath..." -ForegroundColor Yellow
$sig = Set-AuthenticodeSignature -FilePath $ExePath -Certificate $cert

if ($sig.Status -eq "Valid") {
    Write-Host ""
    Write-Host ">>> [SUCCESS] Digital signature applied successfully!" -ForegroundColor Green
    Write-Host "    Status: $($sig.Status)" -ForegroundColor Green
    Write-Host "    Signer: $($sig.SignerCertificate.Subject)" -ForegroundColor Green
    Write-Host "    Publisher will now show as verified ('확인된 게시자: KidInternetLock')." -ForegroundColor Cyan
} else {
    Write-Host "[!] Digital signature status: $($sig.Status) - $($sig.StatusMessage)" -ForegroundColor Yellow
}
Write-Host "==========================================================" -ForegroundColor Cyan
