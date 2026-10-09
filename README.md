# Ping Notifier

English | [日本語](README.ja.md)

A menu bar app (system tray app on Windows) that detects ping timeouts (packet loss) and shows them as OS notifications.

- The icon color shows the status: green = OK / red = packet loss / gray = paused / orange = cannot run ping
- Notifications are sent only when packet loss starts and when the connection is restored (no repeated notifications during an outage)
- Updates itself automatically when a new version is released (see below)
- "Launch at Login" can be toggled from the menu (macOS 13 or later / Windows)
- Available in English and Japanese (follows the OS language; can also be set explicitly)

## Install with Homebrew (macOS)

```sh
brew install --cask novalumo/tap/ping-notifier
```

The cask is in [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap). The app updates itself, so `brew upgrade` skips it unless you pass `--greedy`.

## Install with Nix (macOS, Apple Silicon)

```sh
nix run github:siraken/ping-notifier          # try it without installing
nix profile add github:siraken/ping-notifier  # install
```

The flake builds the app from source. `$out/Applications/Ping Notifier.app` is the app itself and `$out/bin/ping-notifier` launches it. With nix-darwin or Home Manager, add the flake as an input and put `inputs.ping-notifier.packages.${pkgs.system}.default` in `environment.systemPackages` or `home.packages`.

Notes:

- The app does not replace itself, because the Nix store is read-only. Update with Nix (e.g. `nix profile upgrade ping-notifier` or `nix flake update`). The app still tells you when a new version is released
- The app's path changes with every update, so "Launch at Login" may need to be turned on again after updating
- Do not install it together with the Homebrew or downloaded version (they share the same bundle ID)

## Download

Download the file for your OS from [Releases](https://github.com/siraken/ping-notifier/releases).

| OS | File |
| --- | --- |
| macOS (Apple Silicon) | `PingNotifier-<version>-macos-arm64.zip` |
| Windows (x64) | `PingNotifier-<version>-windows-x64.zip` |

Versions that are not signed and notarized show an OS warning on first launch. See the release notes (`.github/release-notes.md`) for how to proceed. Once installed, the app keeps itself up to date.

## Release process

1. Update `version` in `Cargo.toml` and commit
2. Push a tag with the same version (e.g. `git tag v0.2.0 && git push origin v0.2.0`)
3. `.github/workflows/release.yml` builds for macOS and Windows, creates a GitHub Release, and updates `Casks/ping-notifier.rb` in [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) (see below)

The workflow fails if the tag does not match `version` in `Cargo.toml`. Running it manually (workflow_dispatch) from the Actions page builds without creating a release; the build outputs are available as artifacts of the run.

## Automatic updates

The app checks the latest version on [Releases](https://github.com/siraken/ping-notifier/releases) 30 seconds after launch and every 6 hours after that. When a new version is found, it is verified as follows before the app replaces itself and restarts.

1. The hash of the downloaded file matches `SHA256SUMS` attached to the release
2. macOS only: the bundle ID of the extracted `.app` matches and its code signature is valid. If the running app is signed with a Developer ID, the new version must be signed by the same Team ID

You can also check manually with "Check for Updates" in the menu. Set `auto_update = false` to turn off automatic checks. If replacing the app fails, or when running a development build, the menu offers to open the download page instead.

The GitHub API is accessed without authentication, so the repository must be public.

## macOS signing and notarization (for releases)

When the following GitHub Secrets are set, releases are signed with a Developer ID and notarized by Apple before distribution. Without them, releases are distributed with an ad-hoc signature (Gatekeeper shows a warning on first launch).

| Secret | Contents |
| --- | --- |
| `MACOS_CERTIFICATE_P12` | Base64 of a `.p12` exported from the "Developer ID Application" certificate and its private key |
| `MACOS_CERTIFICATE_PASSWORD` | Password set when exporting the `.p12` |
| `APPLE_API_KEY_P8` | Contents of the App Store Connect API key (`.p8`) |
| `APPLE_API_KEY_ID` | Key ID of the API key |
| `APPLE_API_ISSUER_ID` | Issuer ID of the API key |

Setup:

1. Create a "Developer ID Application" certificate on [Apple Developer](https://developer.apple.com/account/resources/certificates/list) and import it into your keychain (requires the Account Holder role)
2. In Keychain Access, export the certificate together with its private key as a `.p12`
3. Create a team API key on [App Store Connect](https://appstoreconnect.apple.com/access/integrations/api) (access: Developer) and download the `.p8`
4. Register them with GitHub:

```bash
base64 -i DeveloperIDApplication.p12 | gh secret set MACOS_CERTIFICATE_P12
gh secret set MACOS_CERTIFICATE_PASSWORD          # prompts for the value
gh secret set APPLE_API_KEY_P8 < AuthKey_XXXXXXXXXX.p8
gh secret set APPLE_API_KEY_ID --body XXXXXXXXXX
gh secret set APPLE_API_ISSUER_ID --body xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

An ad-hoc signed version can update to a Developer ID signed version. Once the app is Developer ID signed, it only accepts versions signed by the same Team ID.

## Homebrew cask updates (for releases)

After a release is published, the workflow updates `version`, `sha256`, and `url` in `Casks/ping-notifier.rb` of [novalumo/homebrew-tap](https://github.com/novalumo/homebrew-tap) and pushes the change. It writes to the tap with a GitHub App token. If the following secrets are not set, the step is skipped with a warning and the cask must be updated by hand.

| Secret | Contents |
| --- | --- |
| `HOMEBREW_TAP_APP_CLIENT_ID` | Client ID of the GitHub App |
| `HOMEBREW_TAP_APP_PRIVATE_KEY` | Private key (`.pem`) of the GitHub App |

Setup:

1. Create a GitHub App in the novalumo organization settings (Developer settings → GitHub Apps). Turn off Webhook and grant only Repository permissions → Contents: Read and write
2. Generate a private key and download the `.pem`
3. Install the app on the novalumo organization, limited to `homebrew-tap`
4. Register them with GitHub:

```bash
gh secret set HOMEBREW_TAP_APP_CLIENT_ID --body Iv23xxxxxxxxxxxxxxxx
gh secret set HOMEBREW_TAP_APP_PRIVATE_KEY < app-name.2026-10-09.private-key.pem
```

## Build

```bash
nix develop   # or: direnv allow

# Development (notifications are sent as Terminal.app)
cargo run

# macOS: create the .app
cargo bundle --release
codesign --force --sign - "target/release/bundle/osx/Ping Notifier.app"
cp -R "target/release/bundle/osx/Ping Notifier.app" /Applications/
```

macOS asks for notification permission the first time a notification is sent. Choose "Allow".

### App icon

The source is `icons/icon.svg`. After editing it, run `./icons/generate.sh` to regenerate `icons/png/` (macOS) and `icons/icon.ico` (Windows).

## Settings

A settings file is created at the following location on first launch. Edit it via "Open Settings File" in the menu, then choose "Reload Settings" to apply your changes.

- macOS: `~/Library/Application Support/ping-notifier/config.toml`
- Windows: `%APPDATA%\ping-notifier\config.toml`

| Key | Default | Description |
| --- | --- | --- |
| `host` | `"8.8.8.8"` | Host name or IP address to monitor |
| `interval_secs` | `1.0` | Interval between pings (seconds) |
| `timeout_ms` | `1000` | How long to wait for a reply (milliseconds). No reply within this time counts as packet loss |
| `threshold` | `1` | Number of consecutive losses before notifying |
| `notify_recovery` | `true` | Also notify when the connection is restored |
| `auto_update` | `true` | Install new versions automatically |
| `language` | `"auto"` | Display language: `"auto"` (follow the OS) / `"en"` / `"ja"` |

## Display language

The menu and notifications are available in English and Japanese. With `language = "auto"` (the default), the app goes through the OS preferred languages in order and uses the first supported one, falling back to English. You can also set `"en"` or `"ja"`; "Reload Settings" applies the change immediately.

The comments in the settings file created on first launch are written in the display language at that time. Logs and error details are always in English.

## Launch at login

Toggle "Launch at Login" in the menu. The checkmark reflects the registration on the OS side.

- macOS: registered as Ping Notifier in System Settings > General > Login Items (`SMAppService`, macOS 13 or later). If you previously turned it off there, you will be asked for approval; enable it in the settings window that opens
- Windows: registered under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. If you move the exe, turn the option on again to register the new location

It cannot be changed in development builds such as `cargo run` (the menu item is disabled).

## How it works

- Reachability is checked by running the OS `ping` command once per interval (raw ICMP sockets require administrator privileges on some OSes)
- Notifications use [notify-rust](https://crates.io/crates/notify-rust); the menu bar / tray uses [tray-icon](https://crates.io/crates/tray-icon) + [tao](https://crates.io/crates/tao)
- On macOS, notify-rust tries to send notifications as Finder by default, and those are silently dropped. The app therefore sets its own bundle ID (`com.novalumo.ping-notifier`) as the sender at startup (`src/notifier.rs`)

## Notes

- macOS `ping` also limits the overall timeout in whole seconds (`-t`), so `timeout_ms` is effectively rounded up to seconds
- The Windows version is only built and tested in CI and has not been verified on a real machine. Notifications are sent with notify-rust's default identity (PowerShell)
