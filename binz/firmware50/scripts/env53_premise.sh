#!/bin/bash
# ENV-53: premise test at 70%, same session: T A18 A17 A17 A18 T T A17 A18 (production images, --no-ladder).
cd "$(dirname "$0")/.."
T=captures/elf/7450FE24.env37-adv18-cap725-qual.elf; A18=captures/elf/D7256586.a1-adv18-cap725.elf; A17=captures/elf/746ADE45.a2-adv17-shell-pwm.elf
n=0
for side in T A18 A17 A17 A18 T T A17 A18; do
  n=$((n+1)); eval elf=\$$side
  echo "=== $n $side"
  python scripts/bemf_run.py --elf $elf --flash --pre="++++++++++++" --command L --rung-duty 700 --runs 1 --timeout 120 --no-ladder --label env53-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|REFUSED|Error|error:|Traceback" | head -4
done
echo "### ENV-53 DONE"
