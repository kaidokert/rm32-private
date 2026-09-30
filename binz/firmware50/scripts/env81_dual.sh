#!/bin/bash
# ENV-81: half-ring A (8A67E1C4) vs dual-shunt B (56675342) at 750, ABBAAB, sag_run.py.
cd "$(dirname "$0")/.."
A=captures/elf/8A67E1C4.c1-blkring-half.elf; B=captures/elf/56675342.c1-blkring-dual.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== 750 $n $side"
  python scripts/sag_run.py --elf $elf --flash --no-warmup --pre=++++++++++++++ --command L --rung-duty 750 --timeout 170 --end-marker SAGBLKEND --label env81-$n$side 2>&1 | grep -E "VERDICT|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-81 DONE"
