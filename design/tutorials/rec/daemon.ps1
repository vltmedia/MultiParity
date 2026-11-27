# Restart the demo desktop daemon on 127.0.0.1:18529 with a given config folder.
#   pwsh daemon.ps1 <config dir> [device name]      (no config dir: just stop it)
param([string]$Config, [string]$Device = "alice-workstation")
Get-NetTCPConnection -LocalPort 18529 -State Listen -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -Confirm:$false }
if (-not $Config) { exit 0 }
Start-Sleep -Milliseconds 400
$env:MP_CONFIG_DIR = $Config
$env:MP_DEVICE = $Device
$env:MP_DAEMON_DEV_ORIGIN = "http://127.0.0.1:4176"
$exe = "P:\Projects\UAI\MultiParity\Code\multiparity\target\debug\mp.exe"
Start-Process -FilePath $exe -ArgumentList "daemon","--listen","127.0.0.1:18529" -WindowStyle Hidden -RedirectStandardError "$Config\daemon.err.log" -RedirectStandardOutput "$Config\daemon.out.log"
for ($i = 0; $i -lt 50; $i++) { try { Invoke-RestMethod http://127.0.0.1:18529/local/accounts -TimeoutSec 1 | Out-Null; exit 0 } catch { Start-Sleep -Milliseconds 200 } }
Write-Error "daemon did not start"; exit 1
