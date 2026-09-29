#!/bin/bash
# ENV-21: map the 72.5 % edge per event (margin-hist companion 9E5BED16 at 725, x3), then
# the protection sweep on the 70 % image B734ACCD (fresh flash per key, `v` omitted).
cd "$(dirname "$0")/.."
MH=captures/elf/9E5BED16.env20-adv16-cap800-5A-mhist.elf
ELF=captures/elf/B734ACCD.env20-adv16-cap800-5A.elf
for i in 1 2 3; do
  echo "=== mh725 run $i"
  python scripts/bemf_run.py --elf $MH --flash --pre "+++++++++++++" --command L --rung-duty 725 --runs 1 \
    --timeout 120 --no-ladder --label env21-mh725-$i 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -4
done
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder \
    --label env21-prot-$k 2>&1 | grep -E "BEMFRUN|BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -4
done
