# Build the Windows desktop app: installers signed for updates and, when a code-signing
# certificate is set up, signed for Windows too (no SmartScreen "unknown publisher").
#
#   pwsh -File scripts\release\build-desktop.ps1 [-Out <folder>] [-Notes "What changed"]
#
# Code signing (docs/SIGNING.md), whichever is set:
#   Azure Trusted Signing:  AZURE_TRUSTED_SIGNING_ENDPOINT, AZURE_TRUSTED_SIGNING_ACCOUNT,
#                           AZURE_TRUSTED_SIGNING_PROFILE, plus AZURE_CLIENT_ID, AZURE_CLIENT_SECRET,
#                           AZURE_TENANT_ID (needs `cargo install trusted-signing-cli`)
#   A certificate in the Windows certificate store (OV/EV, USB token included):
#                           WINDOWS_CERT_THUMBPRINT
# Neither set: the installers are built unsigned, with a warning.
param(
  [string]$Out = "",
  [string]$Notes = ""
)
$ErrorActionPreference = "Stop"
# Windows PowerShell 5 cannot set an empty environment variable, and the update key's empty
# password must be set (or the build stops and waits for it): use PowerShell 7.5 or later.
if ($PSVersionTable.PSVersion -lt [version]"7.5") {
  if (Get-Command pwsh -ErrorAction SilentlyContinue) {
    & pwsh -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath @PSBoundParameters
    exit $LASTEXITCODE
  }
  throw "This script needs PowerShell 7.5 or later (winget install Microsoft.PowerShell)."
}
$root = Resolve-Path "$PSScriptRoot\..\.."
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# Update signing (always: the app only installs signed updates).
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
  $key = "$env:USERPROFILE\.tauri\multiparity-updater.key"
  if (-not (Test-Path $key)) { throw "No update signing key at $key. See docs/DESKTOP.md." }
  $env:TAURI_SIGNING_PRIVATE_KEY = $key
}
if ($null -eq $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "" }

# Code signing, added to the build's configuration only when set up.
$windows = $null
if ($env:AZURE_TRUSTED_SIGNING_ENDPOINT -and $env:AZURE_TRUSTED_SIGNING_ACCOUNT -and $env:AZURE_TRUSTED_SIGNING_PROFILE) {
  if (-not (Get-Command trusted-signing-cli -ErrorAction SilentlyContinue)) { throw "Azure Trusted Signing is set up but trusted-signing-cli is missing: cargo install trusted-signing-cli" }
  $cmd = "trusted-signing-cli -e $($env:AZURE_TRUSTED_SIGNING_ENDPOINT) -a $($env:AZURE_TRUSTED_SIGNING_ACCOUNT) -c $($env:AZURE_TRUSTED_SIGNING_PROFILE) -d MultiParity %1"
  $windows = @{ signCommand = $cmd }
  Write-Host "Code signing with Azure Trusted Signing ($($env:AZURE_TRUSTED_SIGNING_ACCOUNT)/$($env:AZURE_TRUSTED_SIGNING_PROFILE))."
} elseif ($env:WINDOWS_CERT_THUMBPRINT) {
  $windows = @{ certificateThumbprint = $env:WINDOWS_CERT_THUMBPRINT; digestAlgorithm = "sha256"; timestampUrl = "http://timestamp.digicert.com" }
  Write-Host "Code signing with the certificate $($env:WINDOWS_CERT_THUMBPRINT) from the Windows certificate store."
} else {
  Write-Warning "No code-signing certificate set up: the installers will be unsigned and Windows will warn people (docs/SIGNING.md)."
}

Push-Location "$root\desktop"
try {
  if (-not (Test-Path node_modules)) { pnpm install }
  if ($windows) {
    $config = @{ bundle = @{ windows = $windows } } | ConvertTo-Json -Depth 5 -Compress
    $file = Join-Path $env:TEMP "mp-sign-$PID.json"
    Set-Content -Path $file -Value $config -Encoding utf8
    npx tauri build --config $file
    Remove-Item $file
  } else {
    npx tauri build
  }
  if ($LASTEXITCODE -ne 0) { throw "The build failed." }
} finally { Pop-Location }

$bundle = "$root\desktop\src-tauri\target\release\bundle"
$setup = Get-ChildItem "$bundle\nsis\*-setup.exe" | Sort-Object LastWriteTime | Select-Object -Last 1
$sig = Get-AuthenticodeSignature $setup.FullName
Write-Host "$($setup.Name): code signature $($sig.Status)$(if ($sig.SignerCertificate) { ' by ' + $sig.SignerCertificate.Subject })"

if ($Out) {
  $nodeArgs = @("$root\scripts\release\desktop-latest.mjs", "--out", $Out)
  if ($Notes) { $nodeArgs += @("--notes", $Notes) }
  node @nodeArgs
}
