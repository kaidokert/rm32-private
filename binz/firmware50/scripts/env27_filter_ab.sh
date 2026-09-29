#!/bin/bash
# ENV-27: filter floor 5 (A) vs 3 (B) at 725. Production ABBAAB, then chain ABBAAB.
cd "$(dirname "$0")/.."
PA=captures/elf/A9F121F8.env27-prod-f5-edge725.elf
PB=captures/elf/EC82DAE6.env27-prod-f3-edge725.elf
CA=captures/elf/BE2213DE.env26-chain-origin-refusals-edge725.elf
CB=captures/elf/38CFA5AD.env27-chain-f3-edge725.elf
PLUS=$(printf '+%.0s' $(seq 1 13))
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$PA; [ $side = B ] && elf=$PB
  echo "=== prod $n$side"
  python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L --rung-duty 725 --runs 1 \
    --timeout 120 --no-ladder --label env27-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -3
done
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$CA; [ $side = B ] && elf=$CB
  echo "=== chain $n$side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --pre "$PLUS" --command L --rung-duty 725 \
    --timeout 150 --label env27-chain-$n$side 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-27 DONE"
