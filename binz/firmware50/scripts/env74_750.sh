#!/bin/bash
# ENV-74 (step 6): 750, ABBAAB, A = A6 cap-750 2B9686C5, B = B4w-adv16 cap-750 A83EE981, --no-ladder.
cd "$(dirname "$0")/.."
A=captures/elf/2B9686C5.a6-edge750.elf; B=captures/elf/A83EE981.b4w-adv16-edge750.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== $n $side"
  python scripts/bemf_run.py --elf $elf --flash --pre=++++++++++++++ --command L --rung-duty 750 --runs 1 --timeout 120 --no-ladder --label env74-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-74 DONE"
