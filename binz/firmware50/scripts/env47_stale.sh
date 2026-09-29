#!/bin/bash
# ENV-47: stale-wait (S) vs tag (T) chain captures at 37.5%, S T T S S T T S.
cd "$(dirname "$0")/.."
S=captures/elf/B4273B37.diag-stalewait-chain.elf; T=captures/elf/3B99710D.tag-chain-adv18.elf
n=0
for side in S T T S S T T S; do
  n=$((n+1)); elf=$T; [ $side = S ] && elf=$S
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env47-chain-$n$side 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-47 DONE"
