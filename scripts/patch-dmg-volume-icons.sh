#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping DMG volume icon patching outside macOS"
  exit 0
fi

search_root="${1:-src-tauri/target}"
if [[ ! -d "$search_root" ]]; then
  echo "No Tauri target directory found at $search_root; nothing to patch"
  exit 0
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
found=0
while IFS= read -r -d '' dmg_path; do
  found=1
  "$script_dir/patch-dmg-volume-icon.sh" "$dmg_path"
done < <(find "$search_root" -type f -path '*/bundle/dmg/*.dmg' -print0)

if [[ "$found" -eq 0 ]]; then
  echo "No DMG artifacts found under $search_root; nothing to patch"
fi
