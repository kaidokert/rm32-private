#!/bin/bash
# ENV-48: read-delay (S) vs A1 (T) chain captures at 37.5%, S T T S S T T S.
cd "$(dirname "$0")/.."
S=captures/elf/78EE7143.diag-a1-readdelay-chain.elf; T=captures/elf/970E2A0D.a1-chain-adv18.elf
n=0
for side in S T T S S T T S; do
  n=$((n+1)); elf=$T; [ $side = S ] && elf=$S
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env48-chain-$n$( [ $side = S ] && echo R || echo A) 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-48 DONE"
