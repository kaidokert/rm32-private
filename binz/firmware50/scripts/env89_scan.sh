#!/bin/bash
# ENV-89: scan period 101 (A 8F986323) vs 104 us (B) at 775, ABBAAB, sag_run.py.
cd "$(dirname "$0")/.."
A=captures/elf/8F986323.c3-blkhalf775.elf; B=$(ls captures/elf/*.c3-blkhalf775-s104.elf)
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== 775 $n $side"
  python scripts/sag_run.py --elf $elf --flash --no-warmup --pre=+++++++++++++++ --command L --rung-duty 775 --timeout 170 --end-marker SAGBLKEND --label env89-$n$side 2>&1 | grep -E "VERDICT|REFUSED|Error|Traceback" | head -2
done
echo "### ENV-89 DONE"
