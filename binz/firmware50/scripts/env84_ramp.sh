#!/bin/bash
# ENV-84: ramp step (A F36D7A18) vs fine (B B92546EB), 750, ABBAAB, --no-ladder; both late-worst.
cd "$(dirname "$0")/.."
A=captures/elf/F36D7A18.c3-rampstep.elf; B=captures/elf/B92546EB.c3-rampfine.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== $n $side"
  python scripts/bemf_run.py --elf $elf --flash --pre=++++++++++++++ --command L --rung-duty 750 --runs 1 --timeout 120 --no-ladder --label env84-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -2
done
echo "### ENV-84 DONE"
