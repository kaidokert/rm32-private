#!/bin/bash
# ENV-76 (amended): excursion capture at 750 on 0E443BDB via sag_run.py (full ring dump), 3 runs.
cd "$(dirname "$0")/.."
for i in 1 2 3; do
  echo "=== $i"
  python scripts/sag_run.py --elf captures/elf/0E443BDB.c1-sagfreeze750.elf --flash --no-warmup --pre=++++++++++++++ --command L --rung-duty 750 --timeout 160 --label env76s-$i 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|error:|Traceback|saved|SAGEND" | head -4
done
echo "### ENV-76 DONE"
