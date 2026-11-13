# Changelog

## 2.0.7

### Fixed

- Added English App Center metadata and system-language messages for installation, upgrades, removal, and local password resets.
- Completed language support for shared dialogs, default category labels, RPC address placeholders, sign-in diagnostics, and log timestamps.
- Localized API errors, task recovery warnings, proxy failures, update checks, and engine status messages while preserving original diagnostic data.
- Added English in-app release notes that follow the selected interface language.

## 2.0.6 - 2026-11-12

### Fixed

- Fixed authorized folder refresh failures when paths contain spaces, including Chinese characters, consecutive spaces, and full-width spaces. Original paths and folder safety checks are preserved (#41).
- Fixed normal engine shutdown being treated as a fault after a download error, which could prevent subsequent downloads from starting. Failed tasks retain their error records.

## 2.0.5 - 2026-11-11

### Improved

- Improved redownloading tasks after their files are deleted outside the app, making recovery more efficient and reducing manual steps.

### Fixed

- Fixed externally deleted completed files blocking the task engine from starting, allowing tasks to continue.

## 2.0.4 - 2026-11-09

### Fixed

- Show full names of long-running Rust tests to make debugging and result analysis easier.
- Improved the proxy settings layout and save interaction.
- Fixed truncated English settings menu labels by allowing the sidebar width to adapt.
- Added system language support so wizard screens follow the host language and the user's interface language selection is preserved.

## 2.0.3 - 2026-11-04

### Fixed

- Unified download progress in the task list and details using actual downloaded data. Fixed stale progress after a rollback. Percentages use one decimal place and remain at or below 99.9% until completion.
- Removed misleading 0% progress when total size is unknown, a magnet is resolving, or files await confirmation. Paused, completed, and other inactive tasks no longer show stale speeds or remaining times.

### Improved

- Remaining time supports days, hours, minutes, and seconds, showing up to two adjacent nonzero units and an approximate-time label for long downloads.
- Improved task size, speed, and remaining-time layouts on desktop and mobile. Categories remain in task details to reduce crowding on narrow screens.
- Added a progress bar and downloaded/total sizes to task details. Improved download, file, and technical information layouts and wrapping for long names, links, and paths.

## 2.0.2 - 2026-10-29

### Fixed

- Reinstalling while retaining app data no longer requires entering an existing management password again. Leave both fields blank to keep it. A fresh installation or data without a configured password still requires a valid password.
- Fixed local verification caches being invalidated by changes to the RUST_LOG setting, reducing unnecessary repeated checks.

### Improved

- Installation and upgrade wizards use bold text and line breaks to highlight password behavior, task compatibility, and backup requirements.
- Added an upgrade confirmation checkbox that is unchecked by default. Users must accept the compatibility and backup notice, and the upgrade script checks consent again before replacing files. Version 2.x does not import or resume 1.x tasks. Old records and downloaded files are preserved, along with passwords, settings, folder permissions, and RPC tokens. Back up before upgrading; rolling back requires the matching old package and a complete backup.
- Reuse local verification results for frontend, backend, and scripts when their content and toolchains match, reducing duplicate tests and builds during pushes and local packaging.
- Moved lengthy restart, connection stress, and real shutdown timeout checks into extended verification while retaining core stability checks in daily verification.
- Backend release builds use Thin LTO to reduce package compilation time.

### Documentation

- Clarified reinstalling with retained data, upgrade consent, 1.x task compatibility, and backup and rollback procedures. Added platform references and acceptance requirements for fnOS wizards.
