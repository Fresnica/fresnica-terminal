param(
    [Parameter(Mandatory = $true)][string]$ProviderBinary,
    [Parameter(Mandatory = $true)][string]$ServiceBinary
)

$ErrorActionPreference = "Stop"
$serviceName = "FresnicaSystemAuth"
$installDir = Join-Path $env:ProgramFiles "Fresnica\SystemAuth"
$stateDir = Join-Path $env:ProgramData "Fresnica\SystemAuth"
$installedProvider = Join-Path $installDir "fresnica-system-auth-provider.exe"
$installedService = Join-Path $installDir "fresnica-system-auth-service.exe"

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Run this installer from an elevated PowerShell session."
}

$ProviderBinary = (Resolve-Path $ProviderBinary).Path
$ServiceBinary = (Resolve-Path $ServiceBinary).Path
New-Item -ItemType Directory -Force -Path $installDir, $stateDir | Out-Null

$existing = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
if ($null -ne $existing -and $existing.Status -ne "Stopped") {
    Stop-Service -Name $serviceName -Force
    $existing.WaitForStatus("Stopped", [TimeSpan]::FromSeconds(15))
}

Copy-Item -Force $ProviderBinary $installedProvider
Copy-Item -Force $ServiceBinary $installedService

& icacls.exe $installDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-32-545:(OI)(CI)RX' | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Unable to apply Fresnica System Auth program ACL." }
& icacls.exe $stateDir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Unable to apply Fresnica System Auth state ACL." }

$quotedService = '"' + $installedService + '"'
if ($null -eq $existing) {
    & sc.exe create $serviceName binPath= $quotedService start= auto obj= LocalSystem DisplayName= "Fresnica System Auth" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Unable to create Fresnica System Auth service." }
} else {
    & sc.exe config $serviceName binPath= $quotedService start= auto obj= LocalSystem | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Unable to update Fresnica System Auth service." }
}
& sc.exe description $serviceName "Local Win32 WebAuthn and Windows Hello authorization broker for Fresnica wallet unlock keys." | Out-Null
Start-Service -Name $serviceName
(Get-Service -Name $serviceName).WaitForStatus("Running", [TimeSpan]::FromSeconds(15))

& $installedProvider probe
$probe = $LASTEXITCODE
if ($probe -ne 0 -and $probe -ne 10) {
    throw "Fresnica System Auth provider probe failed with exit $probe."
}

Write-Host "Fresnica Windows System Auth development components installed."
Write-Host "provider: $installedProvider"
Write-Host "service:  $installedService"
Write-Host "state:    $stateDir"
Write-Host "probe:    $probe (0=available, 10=Windows Hello/passphrase fallback unavailable in this session)"
