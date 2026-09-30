#!/bin/bash
# ENV-87: dual (A 7A58666F) vs ovs4 (B F99FA8A7) block rings at 775, ABBAAB, sag_run.py.
cd "$(dirname "$0")/.."
A=captures/elf/7A58666F.c3-blk-dual775.elf; B=captures/elf/F99FA8A7.c3-blk-ovs775.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); eval elf=\$$side
  echo "=== 775 $n $side"
  python scripts/sag_run.py --elf $elf --flash --no-warmup --pre=+++++++++++++++ --command L --rung-duty 775 --timeout 170 --end-marker SAGBLKEND --label env87-$n$side 2>&1 | grep -E "VERDICT|REFUSED|Error|Traceback" | head -2
done
echo "### ENV-87 DONE"
