#!/bin/bash
# ENV-14: protection sweep on B0E5CCD4 at the reset default (25 %), fresh flash per
# key. `v` omitted (ENV-12: it suppresses AverageCurrent foldback and never reaches
# FastBusSag on this motor).
cd "$(dirname "$0")/.."
ELF=captures/elf/B0E5CCD4.env12-adv16-cap675.elf
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder \
    --label env14-prot-$k 2>&1 | grep -E "BEMFRUN|BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -4
done
