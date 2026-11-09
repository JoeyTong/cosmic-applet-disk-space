#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-"$HOME/.local"}
data_dir=${XDG_DATA_HOME:-"$prefix/share"}
app_id=io.github.joeytong.cosmic-applet-disk-space
previous_id=io.github.cosmic_disk_space.Applet
binary=cosmic-applet-disk-space

case "${1:-}" in
    "")
        cargo build --manifest-path "$project_dir/Cargo.toml" --release --locked
        ;;
    --no-build) ;;
    *) printf 'Usage: %s [--no-build]\n' "$0" >&2; exit 2 ;;
esac

target_dir=${CARGO_TARGET_DIR:-"$project_dir/target"}
case "$target_dir" in /*) ;; *) target_dir="$PWD/$target_dir" ;; esac
source_binary="$target_dir/release/$binary"
if [ ! -x "$source_binary" ]; then
    printf 'Release binary not found: %s\n' "$source_binary" >&2
    exit 1
fi
command -v python3 >/dev/null 2>&1 || {
    printf 'python3 is required to generate the desktop launcher.\n' >&2
    exit 1
}

install -d "$prefix/bin" "$data_dir/applications" "$data_dir/icons/hicolor/scalable/apps" \
    "$data_dir/doc/$binary"
# Replace atomically, including when an earlier version is running.
install -m 755 "$source_binary" "$prefix/bin/$binary.new"
mv -f "$prefix/bin/$binary.new" "$prefix/bin/$binary"
install -m 644 "$project_dir/data/icons/drive.svg" \
    "$data_dir/icons/hicolor/scalable/apps/$app_id-symbolic.svg"
install -m 644 "$project_dir/data/icons/README.md" "$data_dir/doc/$binary/icons.md"

python3 - "$project_dir/data/$app_id.desktop" "$data_dir/applications/$app_id.desktop" "$prefix/bin/$binary" <<'PY'
import pathlib
import sys

source, destination, executable = sys.argv[1:]
executable = str(pathlib.Path(executable).absolute())
if any(character in executable for character in "\n\r"):
    raise SystemExit("Installation paths must not contain newlines")
# Exec quoting requires both desktop-entry and command-line escaping.
escapes = {"\\": "\\\\\\\\", '"': '\\\\"', "`": "\\\\`", "$": "\\\\$", "%": "%%"}
quoted = '"' + "".join(escapes.get(character, character) for character in executable) + '"'
contents = pathlib.Path(source).read_text().replace("Exec=cosmic-applet-disk-space", "Exec=" + quoted)
pathlib.Path(destination).write_text(contents)
pathlib.Path(destination).chmod(0o644)
PY

rm -f -- "$data_dir/applications/$previous_id.desktop" \
    "$data_dir/icons/hicolor/scalable/apps/$previous_id-symbolic.svg"

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$data_dir/applications/$app_id.desktop"
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_dir/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache --force --ignore-theme-index "$data_dir/icons/hicolor" >/dev/null 2>&1 || true
fi

printf 'Installed %s\n' "$prefix/bin/$binary"
printf 'Add “Disk Space” in COSMIC Settings → Desktop → Panel → Applets.\n'
