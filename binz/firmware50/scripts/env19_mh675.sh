#!/bin/bash
# ENV-19: per-event arm margin at 675 on the margin-hist companion (owed by the ENV-16/17
# review: the 675 -> 700 step's timing cost was unmeasured per event). Fresh flash per run.
cd "$(dirname "$0")/.."
B=captures/elf/57C3A63C.env16-adv16-cap700-mhist.elf
for i in 1 2 3; do
  echo "=== mh675 run $i"
  python scripts/bemf_run.py --elf $B --flash --pre "+++++++++++" --command L --rung-duty 675 \
    --runs 1 --timeout 120 --no-ladder --label env19-mh675-$i 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -4
done
