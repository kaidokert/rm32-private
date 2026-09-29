#!/bin/bash
# ENV-22: three 725 runs on the sag-capture image; the ring freezes on the first
# AverageCurrent over-block. Fresh flash per run so the `+` climb is absolute.
cd "$(dirname "$0")/.."
ELF=captures/elf/71BE8573.env22-sagcapture-cap800-5A.elf
for i in 1 2 3; do
  echo "=== surge 725 run $i"
  python scripts/sag_run.py --elf $ELF --flash --no-warmup --pre "+++++++++++++" --command L --rung-duty 725 \
    --timeout 150 --label env22-surge725-$i 2>&1 | grep -E "BEMFSELFREF|SAGSNAP|SAGEND|saved|REFUSED|Error|Traceback" | head -6
done
