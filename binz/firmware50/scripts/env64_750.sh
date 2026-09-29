#!/bin/bash
# ENV-64: 750 ABBAAB, A = tag cap-750 D12EB4BE, B = A6 cap-750 2B9686C5 (diagnostic, --no-ladder).
cd "$(dirname "$0")/.."
A=captures/elf/D12EB4BE.env40-adv18-edge750.elf; B=captures/elf/2B9686C5.a6-edge750.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== $n $side"
  python scripts/bemf_run.py --elf $elf --flash --pre="++++++++++++++" --command L --rung-duty 750 --runs 1 --timeout 120 --no-ladder --label env64-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|REFUSED|Error|error:|Traceback" | head -4
done
echo "### ENV-64 DONE"
