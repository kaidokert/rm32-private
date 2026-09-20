#!/usr/bin/env bash
# Flash a binz example and capture N seconds of its RTT output.
# Usage: scripts/run_example.sh <example> [seconds]
# probe-rs is stopped via PowerShell Stop-Process (Git Bash signals do NOT
# reach Windows probe-rs -- see memory: stlink-v21-wedge-recovery).
set -u
EX="$1"
SECS="${2:-8}"
DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROBE="0483:374b:066CFF343433464757233430"
LOG="${TMPDIR:-/tmp}/rtt_${EX}.log"

cd "$DIR"
cargo build --release --example "$EX" 2>&1 | grep -E "^error" && exit 1
probe-rs run --chip STM32G071RBTx --probe "$PROBE" \
  "target/thumbv6m-none-eabi/release/examples/$EX" > "$LOG" 2>&1 &
sleep "$SECS"
powershell -NoProfile -Command "Get-Process probe-rs -ErrorAction SilentlyContinue | Stop-Process -Force" >/dev/null
sleep 1
echo "=== $EX ($SECS s) ==="
cat "$LOG"
