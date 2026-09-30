#!/bin/bash
# ENV-78: block-ring A (B425ADF4) vs ovs4 B at 725 ABBAAB; then ENV-77 750x3 on A. sag_run.py.
cd "$(dirname "$0")/.."
A=captures/elf/B425ADF4.c1-blkring512.elf; B=$(ls captures/elf/*.c1-blkring-ovs4.elf)
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== 725 $n $side"
  python scripts/sag_run.py --elf $elf --flash --no-warmup --pre=+++++++++++++ --command L --rung-duty 725 --timeout 170 --end-marker SAGBLKEND --label env78-$n$side 2>&1 | grep -E "VERDICT|REFUSED|Error|error:|Traceback" | head -3
done
for i in 1 2 3; do
  echo "=== 750 A $i"
  python scripts/sag_run.py --elf $A --flash --no-warmup --pre=++++++++++++++ --command L --rung-duty 750 --timeout 170 --end-marker SAGBLKEND --label env77c-750-$i 2>&1 | grep -E "VERDICT|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-78 DONE"
