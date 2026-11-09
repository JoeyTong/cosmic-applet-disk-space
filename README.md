# COSMIC Disk Space

A native Rust/libcosmic panel applet showing **every local drive’s used space**, refreshed every **2 seconds**.

```text
[SSD icon] 36 / 477 G · 7.5%
[USB icon] 52 / 59 G · 88.1% ⚠
```

## Features

- One compact entry per physical drive: type icon, rounded used/total capacity with a shared unit, and percentage. The used value is bold, with spaces around the slash for readability; drive names appear in the click-open popup.
- Distinct embedded, theme-colored **COSMIC-based icons**: solid SSD enclosures, HDD platters, cabled external variants, a USB symbol for flash drives, a memory-card silhouette, and a neutral unknown-drive enclosure. See [icon sources and attribution](data/icons/README.md).
- **At ≥80% used:** warning-colored background, outline, text, and warning symbol. The threshold uses exact bytes, rather than a rounded percentage.
- Click any drive for a softly accented summary with **large, bold used-space and utilization figures**, plus rounded filesystem cards with bold usage figures, progress bars, total capacity, and available space. Colors follow the native COSMIC light/dark theme. Drive entries have no hover tooltips.
- Expand **Details** for the device path, model, raw physical capacity, filesystem mountpoints, and calculation notes. Details start collapsed each time the popup opens and stay expanded during live refreshes. The header and live-status footer stay visible while the content scrolls.
- Automatic USB connection/removal and mount/unmount detection on the next scan.
- Btrfs subvolumes, bind mounts, and repeated device-mapper paths are counted once. LUKS, LVM, and Linux MD topology is followed to the underlying physical drives.
- Multi-drive filesystems are visibly marked **Shared**. Their complete filesystem usage appears on each member, with an explanation in the popup.
- Unmounted, locked, unavailable, and partially readable drives stay visible. Failed scans retain the last values with a **Stale** label and retry automatically.
- Horizontal panels use a scrollable strip; vertical panels stack drive data. **Shift+mouse wheel**, a horizontal touchpad gesture, or dragging the scrollbar scrolls a crowded horizontal strip. Vertical strips use the normal wheel.

## Build and install

Requires Linux, a COSMIC session, Rust **1.93+**, a C build toolchain, `pkg-config`, Wayland/xkbcommon/fontconfig development libraries, and `lsblk` from util-linux with the `FSSIZE`, `FSUSED`, and `MOUNTPOINTS` JSON columns (util-linux 2.37+). The installer also uses Python 3. Cargo downloads the pinned toolkit dependencies on the first build.

On CachyOS / Arch, the relevant packages are:

```sh
sudo pacman -S --needed base-devel rust pkgconf wayland libxkbcommon fontconfig util-linux python desktop-file-utils
```

Build and install for your user account:

```sh
sh scripts/install.sh
```

Or install an already built release:

```sh
cargo build --release --locked
sh scripts/install.sh --no-build
```

The installer writes:

- `~/.local/bin/cosmic-applet-disk-space`
- `~/.local/share/applications/io.github.joeytong.cosmic-applet-disk-space.desktop`
- `~/.local/share/icons/hicolor/scalable/apps/io.github.joeytong.cosmic-applet-disk-space-symbolic.svg`
- `~/.local/share/doc/cosmic-applet-disk-space/icons.md` (icon attribution and license links)

The desktop launcher contains the absolute binary path, so `~/.local/bin` does not need to be in the panel’s `PATH`. `PREFIX`, `XDG_DATA_HOME`, and `CARGO_TARGET_DIR` are supported; use the same `PREFIX`/`XDG_DATA_HOME` when uninstalling.

### Add to the panel

1. Open **COSMIC Settings → Desktop → Panel** (or right-click the panel and open its settings).
2. Open **Applets / Configure panel applets**, then **Add applet**.
3. Find **Disk Space** and add it to your preferred panel section.

If Settings was already open during installation, close and reopen it to refresh the applet list. The applet is also usable on the COSMIC dock. It requests up to 720 px horizontally or 640 px vertically initially, then follows the space allocated by COSMIC and scrolls when constrained.

## What the numbers mean

The panel rounds capacity values to whole binary units and shows the unit once: `36 / 477 G · 7.5%` means approximately 36 GiB used out of 477 GiB, with the percentage calculated from the exact bytes. Compact suffixes are B, K, M, G, T, P, and E; G means GiB, not decimal GB. The popup uses explicit binary unit names and one decimal place. Rounding affects presentation only; the ≥80% warning uses exact byte counts.

`used / total` sums **mounted filesystem capacity**, not raw drive size. Unmounted partitions, swap, and unallocated space are excluded. Raw physical capacity is shown separately under **Details** in the popup. `used` follows the kernel’s filesystem usage (`f_blocks − f_bfree`), while “available to applications” uses `f_bavail`; reserved blocks can make these differ. This means the percentage can differ from `df`’s rounded, available-space-based percentage. The percentage is truncated to one decimal place so it never rounds up to 80% below the warning threshold.

Btrfs deduplication uses its filesystem UUID. Other filesystems use block-device identity, avoiding accidental merging of cloned UUIDs. A shared RAID/LVM/Btrfs filesystem’s figures describe the **whole filesystem**, not allocation to a particular physical member; do not add those shared figures across drives.

Device classification uses transport, rotational/removable flags, and udev’s flash/card hints. A USB bridge that hides the media type can make a flash drive look like an external SSD. No SMART polling, disk-content traversal, mounting, unlocking, or root access is used. Network shares, remote block transports, loop devices, RAM disks, and optical drives are excluded. Devices with filesystems that `lsblk` cannot associate with a mounted block device (such as ZFS members) remain visible as **Unavailable**.

## Diagnostics and development

Print a live JSON snapshot without creating any window:

```sh
~/.local/bin/cosmic-applet-disk-space --dump
```

Run storage tests without compiling the GUI, or run the same scanner independently:

```sh
cargo test --locked --no-default-features
cargo run --locked --no-default-features -- --dump
```

Full checks:

```sh
cargo fmt --all -- --check
cargo clippy --release --locked --all-targets -j 4 -- -D warnings
cargo build --release --locked
desktop-file-validate data/io.github.joeytong.cosmic-applet-disk-space.desktop
```

`Cargo.lock` pins libcosmic to `430f3d48a97d8779f9c1985473912e594ccca1ae`, the toolkit revision used by COSMIC applets 1.11.0. Keep `--locked` for reproducible builds.

The applet runs one asynchronous `lsblk --json --bytes --tree` scan at a time, with a five-second timeout and cancellation of a timed-out child. Filesystem statistics are collected by that child process; filesystem I/O never blocks rendering. A small blocking worker reads sysfs/udev metadata. Normal updates run every two seconds; slow or failing devices can delay a reading and are reported explicitly.

Source layout:

- `src/storage.rs`: topology, usage accounting, device classification, and scanner.
- `src/storage_tests.rs`: Btrfs, LUKS/LVM/RAID, hot-plug, warning-boundary, and error-state regressions.
- `src/app.rs`: panel strip, warning styling, refresh lifecycle, and popup.
- `src/icons.rs`, `data/icons/`: embedded symbolic icons.
- `scripts/`: per-user installation and removal.

## Uninstall

Remove Disk Space from Panel Settings, then run:

```sh
sh scripts/uninstall.sh
```
