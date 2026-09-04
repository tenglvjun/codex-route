#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <path-to-dmg>" >&2
  exit 2
fi

dmg_path="$1"
if [[ ! -f "$dmg_path" ]]; then
  echo "DMG not found: $dmg_path" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "DMG volume icon patching is only supported on macOS" >&2
  exit 2
fi

for command_name in hdiutil find mktemp stat cmp; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Required command is not available: $command_name" >&2
    exit 2
  fi
done

dmg_dir="$(cd "$(dirname "$dmg_path")" && pwd -P)"
dmg_file="$(basename "$dmg_path")"
dmg_path="$dmg_dir/$dmg_file"
original_mode="$(stat -f '%Lp' "$dmg_path")"
tmp_dir="$(mktemp -d "$dmg_dir/.codex-route-dmg-patch.XXXXXX")"
mount_dir="$tmp_dir/mount"
rw_image="$tmp_dir/read-write.dmg"
patched_image="$tmp_dir/patched.dmg"
device=""
mkdir "$mount_dir"

cleanup() {
  if [[ -n "$device" ]]; then
    hdiutil detach "$device" >/dev/null 2>&1 || true
  fi
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

echo "Converting $dmg_file to a temporary writable image"
hdiutil convert "$dmg_path" -format UDRW -o "$rw_image" >/dev/null

attach_output="$(hdiutil attach -nobrowse -readwrite -mountpoint "$mount_dir" "$rw_image")"
device="$(printf '%s\n' "$attach_output" | awk '$1 ~ /^\/dev\/[^[:space:]]+$/ { print $1; exit }')"
if [[ -z "$device" ]]; then
  echo "Unable to determine mounted DMG device" >&2
  exit 1
fi

volume_icon="$mount_dir/.VolumeIcon.icns"
changed=0
app_path="$(find "$mount_dir" -maxdepth 1 -type d -name '*.app' -print -quit)"
if [[ -z "$app_path" ]]; then
  echo "No .app bundle found in mounted DMG" >&2
  exit 1
fi
app_icon="$app_path/Contents/Resources/icon.icns"
if [[ ! -f "$app_icon" ]]; then
  echo "No app icon found at $app_icon" >&2
  exit 1
fi

if [[ -f "$volume_icon" ]] && cmp -s "$volume_icon" "$app_icon"; then
  rm "$volume_icon"
  changed=1
  echo "Removed application-derived volume icon"
elif [[ -f "$volume_icon" ]]; then
  echo "DMG has a custom volume icon; leaving it unchanged"
else
  echo "DMG has no custom volume icon; leaving it unchanged"
fi

sync
hdiutil detach "$device" >/dev/null
device=""

if [[ "$changed" -eq 0 ]]; then
  echo "No application-derived volume icon found; leaving $dmg_file unchanged"
  exit 0
fi

echo "Compressing the patched image"
hdiutil convert "$rw_image" -format UDZO -imagekey zlib-level=9 -o "$patched_image" >/dev/null
if [[ ! -f "$patched_image" ]]; then
  echo "Patched DMG was not created" >&2
  exit 1
fi

mv "$patched_image" "$dmg_path"
chmod "$original_mode" "$dmg_path"
echo "Patched $dmg_path"
