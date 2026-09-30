#!/bin/bash
# ENV-88: split-half at 775 (8F986323) x3; then the C3 storm-reset bench check (u, then 500 without reflash).
cd "$(dirname "$0")/.."
for i in 1 2 3; do
  echo "=== 775 half $i"
  python scripts/sag_run.py --elf captures/elf/8F986323.c3-blkhalf775.elf --flash --no-warmup --pre=+++++++++++++++ --command L --rung-duty 775 --timeout 170 --end-marker SAGBLKEND --label env88-$i 2>&1 | grep -E "VERDICT|REFUSED|Error|Traceback" | head -2
done
echo "=== storm then normal (no reflash)"
python scripts/bemf_run.py --elf captures/elf/FC76E4B6.c3-prod.elf --flash --command u --runs 1 --timeout 120 --no-ladder --label env88-u 2>&1 | grep -E "BEMFINJECT|REFUSED|Error|Traceback" | head -2
python scripts/bemf_run.py --elf captures/elf/FC76E4B6.c3-prod.elf --pre=++++ --command L --rung-duty 500 --runs 1 --timeout 120 --no-ladder --label env88-after-u 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback|BEMFDONE" | head -3
echo "### ENV-88 DONE"
