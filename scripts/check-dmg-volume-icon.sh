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

mount_dir="$(mktemp -d -t codex-route-dmg-check.XXXXXX)"
device=""
cleanup() {
  if [[ -n "$device" ]]; then
    hdiutil detach "$device" >/dev/null 2>&1 || true
  fi
  rmdir "$mount_dir" 2>/dev/null || true
}
trap cleanup EXIT

attach_output="$(hdiutil attach -nobrowse -readonly -mountpoint "$mount_dir" "$dmg_path")"
device="$(printf '%s\n' "$attach_output" | awk '$1 ~ /^\/dev\/[^[:space:]]+$/ { print $1; exit }')"
if [[ -z "$device" ]]; then
  echo "Unable to determine mounted DMG device" >&2
  exit 1
fi

app_path="$(find "$mount_dir" -maxdepth 1 -type d -name '*.app' -print -quit)"
if [[ -z "$app_path" ]]; then
  echo "No .app bundle found in mounted DMG" >&2
  exit 1
fi

volume_icon="$mount_dir/.VolumeIcon.icns"
app_icon="$app_path/Contents/Resources/icon.icns"
if [[ ! -f "$app_icon" ]]; then
  echo "No app icon found at $app_icon" >&2
  exit 1
fi

if [[ -f "$volume_icon" ]] && cmp -s "$volume_icon" "$app_icon"; then
  echo "DMG volume icon incorrectly matches the application icon: $volume_icon" >&2
  exit 1
fi

echo "DMG volume icon is independent from the application icon"
