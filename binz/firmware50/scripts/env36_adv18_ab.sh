#!/bin/bash
# ENV-36: advance 16 (A) vs 18 (B) at 725. Production ABBAAB, then chain ABBAAB.
cd "$(dirname "$0")/.."
PA=captures/elf/A9F121F8.env27-prod-f5-edge725.elf
PB=captures/elf/C567066B.env36-prod-adv18-edge725.elf
CA=captures/elf/BE2213DE.env26-chain-origin-refusals-edge725.elf
CB=captures/elf/C2F405EA.env36-chain-adv18-edge725.elf
PLUS=$(printf '+%.0s' $(seq 1 13))
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$PA; [ $side = B ] && elf=$PB
  echo "=== prod $n$side"
  python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L --rung-duty 725 --runs 1 \
    --timeout 120 --no-ladder --label env36-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -3
done
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$CA; [ $side = B ] && elf=$CB
  echo "=== chain $n$side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --pre "$PLUS" --command L --rung-duty 725 \
    --timeout 150 --label env36-chain-$n$side 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-36 DONE"
