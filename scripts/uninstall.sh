#!/bin/sh
set -eu

prefix=${PREFIX:-"$HOME/.local"}
data_dir=${XDG_DATA_HOME:-"$prefix/share"}
app_id=io.github.joeytong.cosmic-applet-disk-space
previous_id=io.github.cosmic_disk_space.Applet

rm -f -- "$prefix/bin/cosmic-applet-disk-space" \
    "$data_dir/applications/$app_id.desktop" \
    "$data_dir/applications/$previous_id.desktop" \
    "$data_dir/icons/hicolor/scalable/apps/$app_id-symbolic.svg" \
    "$data_dir/icons/hicolor/scalable/apps/$previous_id-symbolic.svg" \
    "$data_dir/doc/cosmic-applet-disk-space/icons.md"
rmdir -- "$data_dir/doc/cosmic-applet-disk-space" 2>/dev/null || true

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_dir/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache --force --ignore-theme-index "$data_dir/icons/hicolor" >/dev/null 2>&1 || true
fi
printf 'Disk Space uninstalled. Remove its entry from COSMIC Panel Settings if present.\n'
