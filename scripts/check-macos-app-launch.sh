#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <path-to-app-executable>" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping macOS app launch check outside macOS"
  exit 0
fi

app_executable="$1"
if [[ ! -x "$app_executable" ]]; then
  echo "App executable is missing or not executable: $app_executable" >&2
  exit 2
fi

log_file="$(mktemp -t codex-route-launch-check.XXXXXX)"
pid=""
cleanup() {
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill -TERM "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  fi
  rm -f "$log_file"
}
trap cleanup EXIT

RUST_BACKTRACE=1 "$app_executable" >"$log_file" 2>&1 &
pid=$!

for _ in {1..20}; do
  if ! kill -0 "$pid" 2>/dev/null; then
    cat "$log_file" >&2
    echo "macOS app exited during startup" >&2
    exit 1
  fi
  sleep 0.25
done

if grep -Eiq 'panic|there is no reactor running|abort' "$log_file"; then
  cat "$log_file" >&2
  echo "macOS app reported a startup failure" >&2
  exit 1
fi

echo "macOS app stayed running during startup"
