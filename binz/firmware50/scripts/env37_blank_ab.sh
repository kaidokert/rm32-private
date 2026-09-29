#!/bin/bash
# ENV-37: BLANK_ARM_MIN_US 16 (A) vs 12 (B), adv 16 at 725. Production ABBAAB, then chain ABBAAB.
cd "$(dirname "$0")/.."
PA=captures/elf/1E5C79B4.env37-A-adv16-blank16-cap725.elf
PB=captures/elf/B71561C2.env37-B-adv16-blank12-cap725.elf
CA=captures/elf/BE2213DE.env26-chain-origin-refusals-edge725.elf
CB=captures/elf/C2F405EA.env36-chain-adv18-edge725.elf
PLUS=$(printf '+%.0s' $(seq 1 13))
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$PA; [ $side = B ] && elf=$PB
  echo "=== prod $n$side"
  python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L --rung-duty 725 --runs 1 \
    --timeout 120 --no-ladder --label env37-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-37 DONE"
