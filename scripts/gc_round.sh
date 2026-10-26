#!/usr/bin/env bash
# One plan section 3 reference run (As-run strings) for a gc step.
#
#   scripts/gc_round.sh <step> <board> <r1|r2|r3|r4>
#   scripts/gc_round.sh <step> flash-binz      (once per step, before binz runs)
#
# Routes the relay to <board>, flies one run of captures/gc/<step>/<board>.elf
# (the L431 path reflashes + resets every run via l431_ref_run.sh, as the
# L431 reference was built), names it <step>-<rN>-<kind>. One run per call:
# runs stay strictly sequential and each hold is one command.
set -eu
cd "$(dirname "$0")/../rm32_stm32"
step=${1:?step}; board=${2:?board}
elf=../captures/gc/$step
binz_probe=0483:374b:066CFF343433464757233430
if [ "$board" = flash-binz ]; then
    probe-rs download --chip STM32G071RBTx --probe $binz_probe "$elf/binz.elf"
    probe-rs reset --chip STM32G071RBTx --probe $binz_probe
    exit 0
fi
r=${3:?r1..r4}
case "$board:$r" in
    l431:r1) name=r1-ladder rec=slow prof=15:5,20:5,30:5,40:5,50:5,60:5,70:5,80:5 extra= ;;
    l431:r2) name=r2-start  rec=fast prof=15:4 extra=--from-arm ;;
    l431:r3) name=r3-step   rec=fast prof=15:3,60:4,40:0.35,25:0.35,15:0.35,10:4 extra= ;;
    l431:r4) name=r4-top    rec=slow prof=15:2,20:2,30:2,40:2,50:2,60:2,70:2,80:15 extra= ;;
    binz:r1) name=r1-ladder rec=slow prof=10:5,20:5,30:5,40:5,50:5,60:5,70:5 extra= ;;
    binz:r2) name=r2-start  rec=fast prof=10:4 extra=--from-arm ;;
    binz:r3) name=r3-step   rec=fast prof=10:3,60:4,40:0.35,25:0.35,15:0.35,10:4 extra= ;;
    binz:r4) name=r4-top    rec=slow prof=10:2,20:2,30:2,40:2,50:2,60:2,70:15 extra= ;;
    *) echo "unknown board/round $board $r" >&2; exit 2 ;;
esac
if [ "$board" = l431 ]; then
    bash scripts/l431_ref_run.sh "$elf/l431.elf" "$step-$name" $rec $prof $extra
else
    python ../scripts/relay.py set binz
    python ../scripts/run_capped.py --secs 115 -- python scripts/binz_profile.py --board binz \
        --name "$step-$name" --rec $rec --profile $prof $extra | tail -2
fi
