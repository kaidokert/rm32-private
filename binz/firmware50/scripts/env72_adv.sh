#!/bin/bash
# ENV-72: B4w advance 18 (A, CE132584) vs 16 (B, 4A4A4369) at 700, ABBAAB, --no-ladder.
cd "$(dirname "$0")/.."
A=captures/elf/CE132584.b4w-prod.elf; B=captures/elf/4A4A4369.b4w-adv16.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== $n $side"
  python scripts/bemf_run.py --elf $elf --flash --pre=++++++++++++ --command L --rung-duty 700 --runs 1 --timeout 120 --no-ladder --label env72-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-72 DONE"
