#!/usr/bin/env bash
# One bisect run on L431: flash an image, ladder, then slam.
#   scripts/bisect_run.sh <elf> <logdir> <tag>
set -u
elf=$1; d=$2; tag=$3
cd "$(dirname "$0")"
mkdir -p "$d"
date +%T
bash requal_flash.sh l431 "$elf" 2>&1 | tail -1
timeout 240 python bf_ladder.py --board l431 --esclog "$d/${tag}_ladder.log" 2>&1 | tail -1
timeout 200 python bf_slam.py --board l431 --esclog "$d/${tag}_slam.log" 2>&1 | grep -a 'verdict'
grep -a '\[run\]' "$d/${tag}_slam.log" | tail -1 | grep -ao ' dsy=[0-9]*\| fast=[0-9]*\| slow=[0-9]*\| orbit=[0-9]*\| comms=[0-9]*' | tr -d '\n'; echo
date +%T
