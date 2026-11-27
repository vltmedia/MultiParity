# The desktop app: building, updates, downloads

Each MultiParity server hands out the desktop app itself: people download it from
**Get the desktop app** (account menu on the web), and the app updates itself from the servers
it is signed in to. A studio decides which version its people run by what it puts on its server.

## Building

On each system you ship for (Windows, a Mac; Linux from the build host):

    cd desktop
    pnpm install
    TAURI_SIGNING_PRIVATE_KEY=~/.tauri/multiparity-updater.key TAURI_SIGNING_PRIVATE_KEY_PASSWORD= pnpm tauri build

On Windows (PowerShell):

    $env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\multiparity-updater.key"
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
    pnpm tauri build

The output is in `desktop/src-tauri/target/release/bundle/`: the installers (`nsis/*-setup.exe`,
`msi/`, `dmg/`, `appimage/`) and, for updates, each update bundle with a `.sig` next to it.

### The update signing key

Updates are only installed when they are signed with the key whose public half is in
`desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). The private key lives at
`~/.tauri/multiparity-updater.key` on the machine that made it, **not** in the repository.

- Back it up somewhere safe (a password manager's file storage). Lose it and installed apps
  can no longer be updated: people would have to download a new version by hand.
- Copy it to each build machine (the Mac too) that signs releases.
- It has no password. To add one: `npx tauri signer generate -w ~/.tauri/multiparity-updater.key`
  makes a new pair; put the new public key in `tauri.conf.json`, and set
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when building.

This is separate from code signing (Windows SmartScreen, macOS Gatekeeper), which is about the
operating system trusting the installer: see [SIGNING.md](SIGNING.md).

## Publishing a version to a server

1. Raise `version` in `desktop/src-tauri/tauri.conf.json` (and `desktop/src-tauri/Cargo.toml`),
   then build on each system.
2. Collect each build into one folder; run it on every build machine into the same folder (copy
   the folder between machines), so `latest.json` lists every platform:

       node scripts/release/desktop-latest.mjs --out ~/mp-desktop --notes "What changed"

3. Copy the folder's contents to the server's desktop folder. With the bundled Docker volume:

       docker compose cp ~/mp-desktop/. server:/data/desktop/

   or set `MP_DESKTOP_FILES=/srv/multiparity/desktop` in `.env` (a host folder) and copy there.

The server reads the folder on every request, so there is nothing to restart.
`GET /api/desktop` shows what is published; apps ask
`/api/desktop/update/{target}/{arch}/{version}` within a minute of starting and every six hours.

## What people see

- A newer version is downloaded in the background. A system notification says it is ready, and
  the tray menu and the app's sidebar show **Restart to update to 0.2.0**. Nothing restarts
  until they choose it.
- The sidebar shows the running version and **Check for updates**.
- On Windows the installer runs quietly with a progress bar and starts the app again.
