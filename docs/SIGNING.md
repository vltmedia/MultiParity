# Code signing the desktop app

Without code signing, Windows shows "Windows protected your PC / Unknown publisher" and macOS
says the app "cannot be opened because the developer cannot be verified". Signing tells the
operating system who made the installer. It is separate from the **update** signature
(docs/DESKTOP.md), which the app itself checks and which is already set up.

The build scripts sign automatically once credentials are present, and build unsigned (with a
warning) until then:

    pwsh -File scripts\release\build-desktop.ps1 -Out C:\mp-desktop    # Windows (PowerShell 7.5+)
    scripts/release/build-desktop.sh --out ~/mp-desktop --universal                                   # Mac

## Windows

Pick one.

### Azure Trusted Signing (recommended)

About US$10 a month, no hardware token, and installers are trusted by SmartScreen straight away.
Available to organizations with three or more years of history (and to individuals in the US
and Canada; check Microsoft's current eligibility).

1. In the Azure portal create a **Trusted Signing account** and a **certificate profile** of
   type *Public Trust*. Identity validation takes a few days.
2. Create an app registration (service principal), give it the *Trusted Signing Certificate
   Profile Signer* role on the account, and make a client secret.
3. On the build machine: `cargo install trusted-signing-cli`, then set
   - `AZURE_TRUSTED_SIGNING_ENDPOINT` (e.g. `https://eus.codesigning.azure.net`)
   - `AZURE_TRUSTED_SIGNING_ACCOUNT`, `AZURE_TRUSTED_SIGNING_PROFILE`
   - `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`

### A code-signing certificate (OV or EV)

Bought from a certificate authority (Sectigo, DigiCert, SSL.com ...), about US$200 to $600 a
year. Since 2023 these come on a hardware token or a cloud HSM, never as a plain file.

1. Install the token's driver so the certificate shows in the Windows certificate store
   (`certmgr.msc` > Personal > Certificates).
2. Copy its thumbprint (certificate > Details > Thumbprint) and set
   `WINDOWS_CERT_THUMBPRINT` to it. The build signs with `signtool` and timestamps it, so the
   signature stays valid after the certificate expires.

An OV certificate builds SmartScreen reputation over the first few hundred downloads; EV and
Trusted Signing are trusted from the start.

Check a build: right-click the installer > Properties > Digital Signatures, or the build
script's last line (`code signature Valid by CN=...`).

## macOS

1. Join the **Apple Developer Program** (US$99 a year) as an organization.
2. In Xcode > Settings > Accounts (or developer.apple.com > Certificates) create a
   **Developer ID Application** certificate. It goes into the Mac's keychain.
3. For notarization (Apple's malware check, needed since macOS 10.15), create an
   **App Store Connect API key** (Users and Access > Integrations > Keys, role Developer) and
   download the `.p8` file.
4. On the Mac that builds, set
   - `APPLE_SIGNING_IDENTITY="Developer ID Application: Your Studio (TEAMID)"`
     (`security find-identity -v -p codesigning` lists it)
   - `APPLE_API_ISSUER`, `APPLE_API_KEY` (the key id) and `APPLE_API_KEY_PATH` (the `.p8`)
   - or instead of the API key: `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and
     `APPLE_TEAM_ID`.

The build signs the app with the hardened runtime, sends it to Apple, waits for the answer and
staples the ticket. `--universal` builds one app for Apple silicon and Intel Macs.

Check a build: `codesign -dv --verbose=2 MultiParity.app` and
`spctl -a -vv MultiParity.app` (should say *accepted, source=Notarized Developer ID*).

## Keep secrets out of the repository

Certificates, tokens, passwords and the update key stay on the build machines (or in a CI
secret store), set as environment variables when building. Nothing in the repository holds them.
