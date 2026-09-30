#!/bin/bash
# ENV-77: per-block ring on 2D2909D2 at 750 x3 and 725 x2 (same session), via sag_run.py.
cd "$(dirname "$0")/.."
for spec in 750:1 725:1 750:2 725:2 750:3; do
  R=${spec%%:*}; i=${spec##*:}
  pre=$(printf '+%.0s' $(seq 1 $(( (R - 400) / 25 ))))
  echo "=== $R $i"
  python scripts/sag_run.py --elf captures/elf/B425ADF4.c1-blkring512.elf --flash --no-warmup --pre=$pre --command L --rung-duty $R --timeout 170 --end-marker SAGBLKEND --label env77b-$R-$i 2>&1 | grep -E "REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-77 DONE"
