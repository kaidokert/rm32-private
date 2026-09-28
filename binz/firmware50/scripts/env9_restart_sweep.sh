#!/bin/bash
# ENV-9: restart 3/3 at 650, then the protection sweep at the reset default
# (25 %), fresh flash before every run so relative shell state is absolute.
cd "$(dirname "$0")/.."
ELF=captures/elf/8FFE35E2.env9-adv16-cap650.elf
for i in 1 2 3; do
  echo "=== restart 650 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre xxxxxx --command Z --rung-duty 650 \
    --runs 1 --timeout 120 --label env9-rst650-$i 2>&1 \
    | grep -E "PROVOKEAT|BEMFRESTART|RESTART|RUN PASS|RUN FAIL|RUNG|NOT RECORDED|REFUSED|Error|Traceback|KILL" | head -12
done
for k in t g f n u h i k q v w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder \
    --label env9-prot-$k 2>&1 | grep -E "BEMFRUN|BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -4
done
