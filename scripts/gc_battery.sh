#!/usr/bin/env bash
# One battery-batch run (plan section 6 step 8) of a battery image set.
#
#   scripts/gc_battery.sh <imgdir> flash-binz      (before that image's binz runs)
#   scripts/gc_battery.sh <imgdir> <l431|binz> <b1|b2|b3>
#
# <imgdir> = captures/gc/<imgdir>/{l431,binz}.elf from gc_battery_images.sh
# (BENCH_SUPPLY=battery). Runs are named <imgdir>-<bN>-<kind>; one run per
# call, strictly sequential, like gc_round.sh. Profiles: binz repeats the
# 2026-10-07 battery qualification slams (slam5-100, slam15a-100) plus a
# 100 % hold; L431 covers its 2026-08 qualified range (15-100 %).
set -eu
cd "$(dirname "$0")/../rm32_stm32"
img=${1:?imgdir}; board=${2:?board}
elf=../captures/gc/$img
binz_probe=0483:374b:066CFF343433464757233430
if [ "$board" = flash-binz ]; then
    probe-rs download --chip STM32G071RBTx --probe $binz_probe "$elf/binz.elf"
    probe-rs reset --chip STM32G071RBTx --probe $binz_probe
    # Wait out the boot (tones, config dump): a run started sooner sees the
    # boot banner during arm and aborts as a reboot.
    python -c "import time; time.sleep(6)"
    exit 0
fi
r=${3:?b1..b3}
case "$board:$r" in
    binz:b1) name=b1-slam5   rec=fast prof=10:2,5:4,100:4,10:4 ;;
    binz:b2) name=b2-slam10  rec=fast prof=10:3,100:4,10:4 ;;
    binz:b3) name=b3-hold    rec=slow prof=10:2,30:2,50:2,70:2,90:2,100:15,10:3 ;;
    l431:b1) name=b1-ladder  rec=slow prof=15:3,20:3,30:3,40:3,50:3,60:3,70:3,80:3,90:3,100:10 ;;
    l431:b2) name=b2-slam15  rec=fast prof=15:3,100:4,15:4 ;;
    l431:b3) name=b3-hold    rec=slow prof=15:2,30:2,50:2,70:2,90:2,100:15 ;;
    *) echo "unknown board/run $board $r" >&2; exit 2 ;;
esac
if [ "$board" = l431 ]; then
    bash scripts/l431_ref_run.sh "$elf/l431.elf" "$img-$name" $rec $prof
else
    python ../scripts/relay.py set binz
    python ../scripts/run_capped.py --secs 115 -- python scripts/binz_profile.py --board binz \
        --name "$img-$name" --rec $rec --profile $prof | tail -2
fi
