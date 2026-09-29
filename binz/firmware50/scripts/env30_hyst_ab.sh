#!/bin/bash
# ENV-30: closed-loop hysteresis 0 (A) vs 1 (B) at 725. Production ABBAAB, then chain ABBAAB.
cd "$(dirname "$0")/.."
PA=captures/elf/A9F121F8.env27-prod-f5-edge725.elf
PB=captures/elf/26DB4CA4.env30-prod-hyst1-edge725.elf
CA=captures/elf/BE2213DE.env26-chain-origin-refusals-edge725.elf
CB=captures/elf/C5B4DDE5.env30-chain-hyst1-edge725.elf
PLUS=$(printf '+%.0s' $(seq 1 13))
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$PA; [ $side = B ] && elf=$PB
  echo "=== prod $n$side"
  python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L --rung-duty 725 --runs 1 \
    --timeout 120 --no-ladder --label env30-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -3
done
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$CA; [ $side = B ] && elf=$CB
  echo "=== chain $n$side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --pre "$PLUS" --command L --rung-duty 725 \
    --timeout 150 --label env30-chain-$n$side 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-30 DONE"
