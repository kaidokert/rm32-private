#!/bin/bash
# ENV-85: event capture at 775 on 5199265B (block ring v3 + freeze-gate), 3 runs.
cd "$(dirname "$0")/.."
for i in 1 2 3; do
  echo "=== 775 $i"
  python scripts/sag_run.py --elf captures/elf/5199265B.c3-evt775.elf --flash --no-warmup --pre=+++++++++++++++ --command L --rung-duty 775 --timeout 170 --end-marker SAGBLKEND --label env85-$i 2>&1 | grep -E "VERDICT|REFUSED|Error|Traceback" | head -2
done
echo "### ENV-85 DONE"
