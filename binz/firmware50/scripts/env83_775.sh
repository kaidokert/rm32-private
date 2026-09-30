#!/bin/bash
# ENV-83: 775 exploration on C2 cap-775 twin F00002E2 x3 (--no-ladder), then block ring 47F14CA8 x1.
cd "$(dirname "$0")/.."
P=+++++++++++++++
for i in 1 2 3; do
  echo "=== 775 $i"
  python scripts/bemf_run.py --elf captures/elf/F00002E2.c2-edge775.elf --flash --pre=$P --command L --rung-duty 775 --runs 1 --timeout 120 --no-ladder --label env83-775-$i 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -2
done
echo "=== 775 blockring"
python scripts/sag_run.py --elf captures/elf/47F14CA8.c2-blkring775.elf --flash --no-warmup --pre=$P --command L --rung-duty 775 --timeout 170 --end-marker SAGBLKEND --label env83-blk775 2>&1 | grep -E "VERDICT|REFUSED|Error|Traceback" | head -2
echo "### ENV-83 DONE"
