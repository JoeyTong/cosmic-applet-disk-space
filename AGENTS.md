# COSMIC Disk Space — contributor guidance

## Project

- Native Rust/libcosmic panel applet for Linux/COSMIC; Rust 1.93+, edition 2024.
- Application ID: `io.github.joeytong.cosmic-applet-disk-space`.
- Binary: `cosmic-applet-disk-space`.
- `Cargo.lock` pins the COSMIC 1.11-generation toolkit. Use `--locked` and avoid unrelated dependency upgrades.
- Confirm ambiguous requirements and significant UI decisions through Q&A before implementation.
- Follow existing Rust patterns and update `README.md` when user-facing behavior changes.

## Source map

- `src/app.rs`: panel layout, popup, theme styles, refresh and popup lifecycle.
- `src/storage.rs`: asynchronous scanner, topology, classification, deduplication, usage formatting.
- `src/storage_tests.rs`: accounting, topology, hot-plug and error-state regression tests.
- `src/icons.rs` and `data/icons/`: embedded, theme-colored device icons.
- `data/icons/README.md`: icon sources, adaptations and attribution; installed with the applet.
- `src/main.rs`: applet startup and the windowless `--dump` command.
- `scripts/install.sh`, `scripts/uninstall.sh`: per-user installation/removal.

## Established behavior

- Panel: drive-type icon, **bold used value**, spaced used/total capacity and percentage, e.g. **36** `/ 477 G · 7.5%`. No drive name or hover tooltip; click opens the popup.
- Use the approved **COSMIC-based icon set C**: SSD enclosure, HDD platter, cabled external variants, USB symbol, memory-card silhouette and neutral fallback. Keep the assets embedded and symbolic; preserve the COSMIC-derived assets' CC BY-SA 4.0 attribution.
- Keep both horizontal and vertical panel layouts, including constrained-space scrolling.
- Units are binary. Compact panel `G` means GiB, not decimal GB. Each panel amount under 10 keeps one decimal place, so small usage is not shown as 0; amounts of 10 or more are whole numbers. Popup capacities use explicit binary units with one decimal place.
- Percentages are truncated to one decimal. The ≥80% warning must compare exact byte counts, never formatted or rounded values. Keep warning symbols and visible stale/partial/shared states.
- Popup: native light/dark theme colors, softly accented summary, rounded filesystem cards, large bold used-space and percentage figures, clearly labeled total and available capacity.
- Device model, raw capacity, mountpoints and calculation notes belong in expandable **Details**. Keep expansion across refreshes; reset on popup close/reopen or selected-drive disconnection.
- Keep the popup bounded and scrollable, with the header and live-status footer accessible.
- Scan every two seconds, with one scan in flight and a five-second timeout. Keep blocking filesystem work off the UI thread and retain last successful values on scan failure.
- Aggregate mounted filesystem capacity, not raw physical capacity. Deduplicate Btrfs subvolumes and repeated mounts; preserve LUKS/LVM/MD relationships and shared-filesystem explanations.
- Preserve unavailable, locked, unmounted and partially readable states rather than presenting missing usage as zero. Available space uses `f_bavail` and can differ from total minus used.
- Storage collection is read-only and does not require root or mount/unlock devices.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --release --locked --all-targets -j 4 -- -D warnings
cargo test --locked --no-default-features
cargo build --release --locked
desktop-file-validate data/io.github.joeytong.cosmic-applet-disk-space.desktop
```

For UI changes, inspect the panel and popup in COSMIC, including scrolling and Details expansion. Keep warning, unavailable and long-label layouts readable. Add meaningful accounting regressions when changing storage logic; cosmetic edits do not need implementation-mirroring tests.

The pinned toolkit uses progress-bar `.girth(...)` and `.class(cosmic::theme::ProgressBar::custom(...))`, text `.class(...)`, and container `.style(...)`. Check the pinned APIs instead of assuming current upstream examples apply.

## Install and activate

```sh
sh scripts/install.sh
# Or, after a successful release build:
sh scripts/install.sh --no-build
~/.local/bin/cosmic-applet-disk-space --dump
```

The installer atomically replaces the executable under `~/.local/bin` and installs the desktop entry and icon under `~/.local/share`. It supports `PREFIX`, `XDG_DATA_HOME` and `CARGO_TARGET_DIR`. Installation alone does not replace a running applet process.

Reloading the COSMIC panel briefly restarts its other applets; obtain user agreement before doing so unless already authorized for the current work. In COSMIC 1.11, signaling only this applet can fail to activate an update. For an authorized panel reload, identify the applet's actual parent panel and verify that `cosmic-session` manages it before sending SIGTERM. Do not use hard-coded PIDs. Verify that the restarted applet uses the installed executable.
