#!/bin/bash
# ENV-17: map the 70 % edge. ABBAAB: A = 675 on the qualified image B0E5CCD4 (re-run of
# earned on the 5 A supply; control for the sag change), B = 700 on the margin-hist
# companion 57C3A63C (per-event wait/left histograms). Fresh flash per run.
cd "$(dirname "$0")/.."
A=captures/elf/B0E5CCD4.env12-adv16-cap675.elf
B=captures/elf/57C3A63C.env16-adv16-cap700-mhist.elf
n=0
for side in A B B A A B; do
  n=$((n+1))
  if [ $side = A ]; then
    echo "=== $n A 675 prod"
    python scripts/bemf_run.py --elf $A --flash --pre "+++++++++++" --command L --rung-duty 675 \
      --runs 1 --timeout 120 --label env17-A675-$n 2>&1 | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -6
  else
    echo "=== $n B 700 mhist"
    python scripts/bemf_run.py --elf $B --flash --pre "++++++++++++" --command L --rung-duty 700 \
      --runs 1 --timeout 120 --no-ladder --label env17-B700mh-$n 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -6
  fi
done
