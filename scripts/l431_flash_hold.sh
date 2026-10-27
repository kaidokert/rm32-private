#!/usr/bin/env bash
# Flash an L431 image (slow SWD: the L431 link on the FC bench fails at
# probe-rs's default clock since 2026-10-27), reset, let it arm, then one
# BF hold through bf_hold.py. One run per invocation.
#   scripts/l431_flash_hold.sh ELF PCT LOG
set -euo pipefail
elf=${1:?elf}; pct=${2:?pct}; log=${3:?log}
probe=0483:374f:0037002F3234510836303532
timeout 200 probe-rs download --speed 100 --chip STM32L431KCUx --probe "$probe" "$elf" 2>&1 | tail -1
timeout 30 probe-rs reset --speed 100 --chip STM32L431KCUx --probe "$probe" >/dev/null 2>&1
sleep 10
python "$(dirname "$0")/bf_hold.py" l431 "$pct" --log "$log"
