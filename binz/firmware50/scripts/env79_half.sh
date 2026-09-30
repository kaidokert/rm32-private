#!/bin/bash
# ENV-79: split-half block ring (8A67E1C4) at 725 x3 and 750 x3, interleaved, same session.
cd "$(dirname "$0")/.."
E=captures/elf/8A67E1C4.c1-blkring-half.elf
for spec in 725:1 750:1 725:2 750:2 725:3 750:3; do
  R=${spec%%:*}; i=${spec##*:}
  pre=$(printf '+%.0s' $(seq 1 $(( (R - 400) / 25 ))))
  echo "=== $R $i"
  python scripts/sag_run.py --elf $E --flash --no-warmup --pre=$pre --command L --rung-duty $R --timeout 170 --end-marker SAGBLKEND --label env79-$R-$i 2>&1 | grep -E "VERDICT|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-79 DONE"
