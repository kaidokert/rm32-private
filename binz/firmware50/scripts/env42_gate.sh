#!/bin/bash
# ENV-42: sag ring frozen at the first hold block >= 4750 mA, 3 runs at 750 on 78D92EB2 (diagnostic).
cd "$(dirname "$0")/.."
ELF=captures/elf/78D92EB2.env42-sagcap-adv18-edge750-freezegate.elf
for i in 1 2 3; do
  echo "=== gate 750 run $i"
  python scripts/sag_run.py --elf $ELF --flash --no-warmup --pre "++++++++++++++" --command L --rung-duty 750 \
    --timeout 150 --label env42-gate750-$i 2>&1 | grep -E "BEMFSELFREF|SAGSNAP|saved|REFUSED|Error|error:|Traceback" | head -6
done
echo "### ENV-42 DONE"
