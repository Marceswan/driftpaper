# Signed macOS releases

The release workflow requires Developer ID signing and Apple notarization. Intel
and Apple Silicon builds are combined before signing with a secure timestamp and
hardened runtime. The bundle identifier remains `com.marcswan.driftpaper`; no
hardened-runtime exceptions are enabled.

The app is signed, notarized, stapled, and checked with Gatekeeper before creating
the ZIP. The DMG is then created, signed, notarized, stapled, and checked separately.
Both submissions must return `Accepted`. Missing credentials, invalid signatures,
rejections, timeouts, and ticket failures stop publication. No unsigned fallback.

## One-time setup

Use an active Apple Developer Program membership and a **Developer ID Application**
certificate with its private key. Apple Development, Apple Distribution, and
Developer ID Installer identities cannot substitute for it.

In Xcode, open Settings > Apple Accounts > your team > Manage Certificates. If the
Developer ID Application certificate says **Not in Keychain**, import its `.p12`
from the Mac where it was created, or create a new Developer ID Application
certificate. Downloading the `.cer` alone does not restore its private key.
Do not revoke existing certificates to configure this project.

Export only the Developer ID Application identity and its private key as a
password-protected `.p12`. Confirm its exact identity using:

```sh
security find-identity -v -p codesigning
```

For notarization, use an App Store Connect **team API key** authorized for the
notary service. Its `.p8`, Key ID, and Issuer ID are separate from the signing
certificate. Find the issuer and key metadata in App Store Connect > Users and
Access > Integrations > App Store Connect API. Individual API keys use different
authentication parameters; this workflow expects a team key.

Configure these repository Actions secrets:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE_P12_BASE64` | Base64-encoded Developer ID identity `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12` |
| `APPLE_SIGNING_IDENTITY` | Exact `Developer ID Application: Name (TEAMID)` identity |
| `APPLE_TEAM_ID` | The 10-character team ID from that signing identity |
| `APPLE_API_KEY_P8_BASE64` | Base64-encoded App Store Connect team API `.p8` |
| `APPLE_API_KEY_ID` | API Key ID |
| `APPLE_API_ISSUER_ID` | Team API Issuer ID |

Upload private files directly to encrypted GitHub Secrets; never commit them or
paste their contents into chat. Replace the example file paths:

```sh
base64 < /path/to/DeveloperID.p12 | gh secret set APPLE_CERTIFICATE_P12_BASE64 --repo Marceswan/driftpaper
base64 < /path/to/AuthKey_KEYID.p8 | gh secret set APPLE_API_KEY_P8_BASE64 --repo Marceswan/driftpaper
# These commands prompt securely for each value.
gh secret set APPLE_CERTIFICATE_PASSWORD --repo Marceswan/driftpaper
gh secret set APPLE_SIGNING_IDENTITY --repo Marceswan/driftpaper
gh secret set APPLE_TEAM_ID --repo Marceswan/driftpaper
gh secret set APPLE_API_KEY_ID --repo Marceswan/driftpaper
gh secret set APPLE_API_ISSUER_ID --repo Marceswan/driftpaper
```

The signing step downloads the Developer ID G1/G2 intermediate certificates from
Apple's PKI service and imports them without changing trust settings. This supplies
the chain that Xcode normally installs on developer Macs. It requires a valid
imported identity before submitting anything to Apple.

The signing step uses an isolated temporary keychain and does not change the
default keychain or search list. An exit trap removes that keychain and private
files on success and failure; jobs run on disposable GitHub-hosted machines.
Secrets are passed only to the configuration check and signing step.

## Run and verify

Push to `main`, or after configuring secrets, run:

```sh
gh workflow run release.yml --ref main --repo Marceswan/driftpaper
```

Manual releases are restricted to `main`. Each Apple submission waits up to 20
minutes; timeouts stop publication even if Apple continues processing. Check the
submission status before retrying. Rejections print the notarization log.

For local signing, supply the same seven environment variables using your secret
manager and pass a finished app and output directory:

```sh
bash release/scripts/sign-notarize-macos.sh target/DriftPaper.app target/signed
```

After downloading and extracting a published ZIP on a Mac:

```sh
codesign --verify --deep --strict --verbose=2 DriftPaper.app
xcrun stapler validate DriftPaper.app
spctl --assess --type execute --verbose=2 DriftPaper.app
```

Launch the downloaded app with Gatekeeper enabled and exercise its menu, wallpaper,
right-click pass-through, and sleep/wake behavior. Run script regression tests with
`python3 release/tests/test_macos_signing.py`. These use mocked Apple commands to
check failure handling, ordering, and cleanup; they do not prove a real signature
or notarization. A real signed release must pass the Apple and Gatekeeper checks.

References: [Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates),
[custom notarization workflows](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow).
