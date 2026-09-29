#!/bin/bash
# ENV-55 diagnostic: chain-origin captures at 700, T A A T T A (T = tag code 2572C4D4, A = A1 code 1B96E7E2).
cd "$(dirname "$0")/.."
T=captures/elf/2572C4D4.tag-chain-origin.elf; A=captures/elf/1B96E7E2.a1-chain-origin.elf
n=0
for side in T A A T T A; do
  n=$((n+1)); eval elf=\$$side
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --pre="++++++++++++" --command L --rung-duty 700 --timeout 150 --label env55-chain-$n$side 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|error:|Traceback" | head -4
done
echo "### ENV-55 DONE"
