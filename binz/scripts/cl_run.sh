#!/usr/bin/env bash
# One-shot closed-loop run: VCOM ring capture + flash/RTT + plot + auto-safe.
# Use the moment the PSU is on. Usage: scripts/cl_run.sh [rtt_secs] [tag]
#   rtt_secs default 55 (walk 20->30% at +8/heartbeat needs ~40 s + margin)
set -u
SECS="${1:-55}"
TAG="${2:-cl_run}"
DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROBE="0483:374b:066CFF343433464757233430"
cd "$DIR"

echo "== build =="
cargo build --release --example closed-loop 2>&1 | grep -E "^error" && exit 1

echo "== VCOM ring listener (COM7) in background =="
python scripts/cl_bemf.py --secs "$((SECS + 8))" --tag "$TAG" > "/tmp/${TAG}_vcom.log" 2>&1 &
VPID=$!
sleep 1

echo "== flash + run closed-loop (${SECS}s), key RTT lines =="
bash scripts/run_example.sh closed-loop "$SECS" 2>&1 \
  | grep -E "VM raw|ENGAGED|CAP ARMED|WALK|REACHED|SUCCESS|KILLED|abort" | tail -60

echo "== waiting for VCOM plot =="
wait "$VPID" 2>/dev/null
cat "/tmp/${TAG}_vcom.log"

echo "== SAFE the board (rtt-hello) =="
probe-rs download --chip STM32G071RBTx --probe "$PROBE" \
  target/thumbv6m-none-eabi/release/examples/rtt-hello 2>&1 | tail -1
probe-rs reset --chip STM32G071RBTx --probe "$PROBE" 2>&1 | tail -1
echo "== done: captures/${TAG}.png =="
