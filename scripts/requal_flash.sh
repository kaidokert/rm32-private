#!/usr/bin/env bash
# Flash one FC-bench ESC with an ELF and let it boot (requal sessions).
#   scripts/requal_flash.sh l431|binz <elf>
# L431 needs --speed 100 on this bench; both get a reset and a boot wait so
# Betaflight's DShot re-detects before the next bf_ladder/bf_slam run.
set -euo pipefail
board=${1:?usage: requal_flash.sh l431|binz <elf>}
elf=${2:?usage: requal_flash.sh l431|binz <elf>}
case "$board" in
    l431) probe=0483:374f:0037002F3234510836303532; chip=STM32L431KCUx; speed="--speed 100"; wait=10 ;;
    binz) probe=0483:374b:066CFF343433464757233430; chip=STM32G071RBTx; speed=""; wait=8 ;;
    *) echo "requal_flash: unknown board '$board'" >&2; exit 2 ;;
esac
echo "flash $board <- $elf ($(sha256sum "$elf" | cut -c1-12))"
timeout 200 probe-rs download $speed --chip "$chip" --probe "$probe" "$elf"
timeout 60 probe-rs reset $speed --chip "$chip" --probe "$probe"
sleep "$wait"
echo "flashed $board"
