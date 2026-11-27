# Set the GitHub Actions secrets that desktop releases need (.github/workflows/desktop.yml), from
# the machines that already build releases: the update key on this PC, and the Developer ID
# certificate and App Store Connect API key on the Mac (its ~/.private_keys/multiparity-notary.env).
# Nothing is printed or saved: each value goes from where it lives straight into `gh secret set`.
#
#   pwsh -File scripts/release/set-github-secrets.ps1 [-Repo vltmedia/MultiParity] [-Mac user@host] [-SkipMac]
#
# Needs `gh` signed in with access to the repository's settings, and SSH to the Mac.
param(
  [string]$Repo = "vltmedia/MultiParity",
  [string]$Mac = "bravomedia@10.0.0.127",
  [switch]$SkipMac
)
$ErrorActionPreference = "Stop"

function Set-Secret([string]$Name, [string]$Value) {
  if (-not $Value) { throw "Nothing to set for $Name." }
  # Through stdin, so the value is never on a command line.
  $Value | gh secret set $Name --repo $Repo
  if ($LASTEXITCODE) { throw "gh could not set $Name." }
  Write-Host "Set $Name"
}

function From-Mac([string]$Script) {
  # The script goes over stdin too; its output is the value.
  $out = $Script.Replace("`r", "") | ssh -o BatchMode=yes $Mac "bash -s"
  if ($LASTEXITCODE) { throw "The Mac could not do that (exit $LASTEXITCODE)." }
  ($out -join "`n").Trim()
}

gh auth status *> $null
if ($LASTEXITCODE) { throw "Sign in to GitHub first: gh auth login" }

# 1. The update signing key, on this PC. Installed apps check every update against it.
$key = Join-Path $env:USERPROFILE ".tauri\multiparity-updater.key"
if (-not (Test-Path $key)) { throw "There is no update key at $key." }
Set-Secret "TAURI_SIGNING_PRIVATE_KEY" (Get-Content -Raw $key).Trim()
$secure = Read-Host "Password of the update key (press Enter if it has none)" -AsSecureString
$plain = [Net.NetworkCredential]::new("", $secure).Password
if ($plain) { Set-Secret "TAURI_SIGNING_PRIVATE_KEY_PASSWORD" $plain } else { Write-Host "No update key password: skipped." }

if ($SkipMac) { Write-Host "Skipped the Mac secrets: the Mac app will be built unsigned."; return }

# 2. Signing and notarizing for macOS, from the Mac's release settings.
$settings = 'set -a; . ~/.private_keys/multiparity-notary.env; set +a'
Set-Secret "APPLE_SIGNING_IDENTITY" (From-Mac "$settings; printf %s `"`$APPLE_SIGNING_IDENTITY`"")
Set-Secret "APPLE_API_ISSUER" (From-Mac "$settings; printf %s `"`$APPLE_API_ISSUER`"")
Set-Secret "APPLE_API_KEY" (From-Mac "$settings; printf %s `"`$APPLE_API_KEY`"")
Set-Secret "APPLE_API_KEY_P8" (From-Mac "$settings; cat `"`$APPLE_API_KEY_PATH`"")

# The Developer ID certificate with its private key. macOS only lets it out with someone at the
# Mac, so export it there first (Keychain Access > the "Developer ID Application" certificate >
# Export… > .p12, with a password) to ~/multiparity-devid.p12. It is deleted from the Mac here
# once GitHub has it.
$p12 = "~/multiparity-devid.p12"
$has = From-Mac "test -f $p12 && echo yes || echo no"
if ($has -ne "yes") {
  throw "There is no $p12 on the Mac. Export the Developer ID Application certificate there (Keychain Access > Export… > .p12) and run this again."
}
Set-Secret "APPLE_CERTIFICATE" ((From-Mac "base64 -i $p12") -replace "\s", "")
$secure = Read-Host "Password you gave the .p12 export" -AsSecureString
Set-Secret "APPLE_CERTIFICATE_PASSWORD" ([Net.NetworkCredential]::new("", $secure).Password)
From-Mac "rm -f $p12" | Out-Null
Write-Host "Deleted $p12 from the Mac."

Write-Host ""
Write-Host "Done. The repository has:"
gh secret list --repo $Repo
