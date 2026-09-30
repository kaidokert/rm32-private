#!/bin/bash
# ENV-70 (step 3): T/S/A attribution at 725 and 750, order T S A A S T T S A per rung, --no-ladder.
cd "$(dirname "$0")/.."
declare -A E725=([T]=captures/elf/7450FE24.env37-adv18-cap725-qual.elf [S]=captures/elf/820B2FAD.b3-stalewait-prod.elf [A]=captures/elf/0A978A82.a6-split-shell-pwm.elf)
declare -A E750=([T]=captures/elf/D12EB4BE.env40-adv18-edge750.elf [S]=captures/elf/FCF3AD62.b3-stalewait-edge750.elf [A]=captures/elf/2B9686C5.a6-edge750.elf)
for R in 725 750; do
  n=0
  for side in T S A A S T T S A; do
    n=$((n+1))
    if [ $R = 725 ]; then elf=${E725[$side]}; pre="+++++++++++++"; else elf=${E750[$side]}; pre="++++++++++++++"; fi
    echo "=== $R $n $side"
    python scripts/bemf_run.py --elf $elf --flash --pre=$pre --command L --rung-duty $R --runs 1 --timeout 120 --no-ladder --label env70-$R-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|error:|Traceback" | head -3
  done
done
echo "### ENV-70 DONE"
