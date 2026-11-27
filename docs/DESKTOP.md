# The desktop app: building, updates, downloads

The desktop app is published as a **GitHub release** of this repository
(https://github.com/vltmedia/MultiParity/releases). Every MultiParity server hands it out from
there: people download it from **Get the desktop app** (account menu on the web), and the app
updates itself through the servers it is signed in to.

A studio that wants to choose the version its people run can put its own build on its server
(below); the server then hands out that one instead of the latest release. Apps that cannot
reach any of their servers check the GitHub release directly.

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

## Publishing a release on GitHub

### With GitHub Actions (recommended)

`.github/workflows/desktop.yml` builds everything on GitHub's machines: Windows (`.exe`, `.msi`),
macOS (one universal app for Apple silicon and Intel) and Linux (`.AppImage`, `.deb`), plus the
`mp` command line for all three, and makes a **draft** release with one `latest.json`.

1. Raise `version` in `desktop/src-tauri/tauri.conf.json`, `desktop/src-tauri/Cargo.toml` and
   `desktop/package.json`, commit and push.
2. Push a tag with that version: `git tag v0.2.0 && git push origin v0.2.0` (or run the workflow
   from the Actions tab; a tag that does not match the version stops it).
3. When it finishes, open the draft under Releases, write the notes, and **Publish**. Installed
   apps are offered the update only then.

It needs these repository secrets (Settings > Secrets and variables > Actions):

| Secret | |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | The contents of the update signing key file (below). Required. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password, if it has one. |
| `APPLE_CERTIFICATE` | The Developer ID Application certificate exported as `.p12`, base64 (`base64 -i cert.p12 \| pbcopy`). |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12`'s export password. |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Name (TEAMID)` |
| `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_P8` | Notarizing with an App Store Connect API key: the issuer id, the key id, and the `.p8` file's contents. |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | Or notarizing with an Apple ID: the Apple ID, an app-specific password, the team id. |

`scripts/release/set-github-secrets.ps1` sets them from the machines that already build releases
(the update key on the Windows PC, the certificate and API key on the Mac), without showing any
of them.

Without the Apple secrets the Mac app is built unsigned (Gatekeeper warns people). Windows builds
are not code signed either way (docs/SIGNING.md).

### By hand

1. Raise `version` in `desktop/src-tauri/tauri.conf.json` and `desktop/src-tauri/Cargo.toml`,
   commit and push.
2. Build on each system (`pwsh -File scripts/release/build-desktop.ps1` on Windows,
   `scripts/release/build-desktop.sh --universal` on a Mac), collecting into one folder with the
   release's address:

       node scripts/release/desktop-latest.mjs --out ~/mp-release --notes "What changed" \
         --base-url https://github.com/vltmedia/MultiParity/releases/download/v0.2.0

3. Make the release with everything in that folder:

       gh release create v0.2.0 ~/mp-release/* --title "MultiParity 0.2.0" --notes "What changed"

   Add files from a second machine later with `gh release upload v0.2.0 <files> --clobber`
   (including the merged `latest.json`).

Updates are checked over `https` or plain `http` (studio servers on the local network often have
no certificate); either way the app installs only what is signed with the update key.

Servers notice a new release within 15 minutes, and apps ask their servers within a minute of
starting and every six hours. `MP_DESKTOP_RELEASES` points a server at another `latest.json`, or
`off` hands out nothing.

## Publishing a version to a server (to pin one)

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
