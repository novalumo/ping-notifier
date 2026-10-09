# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-10-09

### Added

- Simplified Chinese display language (`language = "zh"`). With `"auto"`, Traditional Chinese locales are not matched
- Korean display language (`language = "ko"`)
- Esperanto display language (`language = "eo"`)
- German display language (`language = "de"`)

## [0.5.2] - 2026-10-09

### Fixed

- macOS: the "Updated" notification was not shown after an automatic update. The new version now shows it after restarting (from the next update after this version)

## [0.5.1] - 2026-10-09

### Changed

- The repository moved from `siraken/ping-notifier` to `novalumo/ping-notifier`. Old URLs redirect to the new location, so existing installs keep updating

## [0.5.0] - 2026-10-09

### Added

- Install with Homebrew: `brew install --cask novalumo/tap/ping-notifier`
- Install with Nix: `nix profile add github:novalumo/ping-notifier`. The Nix build does not replace itself; update it with Nix

### Changed

- The macOS download is now `PingNotifier-<version>-macos-arm64.zip` (previously `-macos-universal.zip`)

### Removed

- Support for Intel Macs. The macOS app is built for Apple Silicon only

### Upgrade notes

- v0.4.0 and earlier cannot update to this version automatically because the file name changed. Download it from Releases or install it with Homebrew

## [0.4.0] - 2026-10-09

### Added

- Japanese display language. The app follows the OS language by default; set `language` to `"en"` or `"ja"` to choose one
- The settings file created on first launch is written in the display language

## [0.3.1] - 2026-10-09

### Fixed

- macOS: the app sometimes did not restart after an automatic update and stayed closed

## [0.3.0] - 2026-10-09

### Added

- "Launch at Login" in the menu (macOS 13 or later / Windows)

## [0.2.1] - 2026-10-09

### Changed

- macOS: the app is signed with a Developer ID and notarized by Apple, so Gatekeeper no longer warns on first launch

## [0.2.0] - 2026-10-09

### Added

- Automatic updates from GitHub Releases: checks 30 seconds after launch and every 6 hours, verifies the download against `SHA256SUMS` (and the code signature on macOS), then replaces the app and restarts
- "Check for Updates" and the current version in the menu
- `auto_update` setting

## [0.1.0] - 2026-10-09

### Added

- Menu bar app (system tray app on Windows) that pings a host and notifies you when packet loss starts and when the connection is restored
- Icon color shows the status: OK / packet loss / paused / cannot run ping
- TOML settings file that can be opened and reloaded from the menu

[Unreleased]: https://github.com/novalumo/ping-notifier/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/novalumo/ping-notifier/compare/v0.5.2...v0.6.0
[0.5.2]: https://github.com/novalumo/ping-notifier/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/novalumo/ping-notifier/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/novalumo/ping-notifier/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/novalumo/ping-notifier/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/novalumo/ping-notifier/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/novalumo/ping-notifier/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/novalumo/ping-notifier/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/novalumo/ping-notifier/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/novalumo/ping-notifier/releases/tag/v0.1.0
