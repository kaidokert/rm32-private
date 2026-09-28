#!/bin/bash
# ENV-7: matched A/B at rung 625, advance 16 (A) vs a candidate image (B),
# ABABAB in one session. Every run is freshly flashed so the `+` climb is
# absolute (400 + 9x25 = 625).
#   bash scripts/env7_ab.sh captures/elf/B004C97F.env7-adv18-cap625.elf env7    # flat 18
#   bash scripts/env7_ab.sh captures/elf/527FAAE9.env7-adv20-cap625.elf env7c   # flat 20 control
cd "$(dirname "$0")/.."
A=captures/elf/A09BA143.env6-adv16-cap625-sagclamp.elf
B=${1:?candidate elf}
LABEL=${2:?label prefix}
for i in 1 2 3; do
  for side in A B; do
    elf=$A; [ $side = B ] && elf=$B
    echo "=== $side$i $elf"
    python scripts/bemf_run.py --elf $elf --flash --pre "+++++++++" --command L \
      --rung-duty 625 --runs 1 --timeout 120 --no-ladder --label $LABEL-$side$i 2>&1 \
      | grep -E "BEMFRUN|BEMFDONE|BEMFSELFREF|late_arms|thin|REFUSED|FAIL|Error|Traceback" | head -8
  done
done
