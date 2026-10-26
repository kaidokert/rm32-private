#!/usr/bin/env bash
# One L431 reference run: route the motor, flash an image, reset, fly a
# profile. Exactly one bench run per invocation (never loop this).
#   scripts/l431_ref_run.sh <elf> <name> <rec> <profile> [extra binz_profile args]
set -euo pipefail
elf=$1 name=$2 rec=$3 profile=$4; shift 4
probe=0483:374f:0037002F3234510836303532
python ../scripts/relay.py set l431
probe-rs download --chip STM32L431KCUx --probe "$probe" "$elf" 2>&1 | tail -1
probe-rs reset --chip STM32L431KCUx --probe "$probe"
sleep 3   # boot + arming tune
python ../scripts/run_capped.py --secs 115 -- python scripts/binz_profile.py \
    --board l431 --name "$name" --rec "$rec" --profile "$profile" "$@" 2>&1 | tail -1
