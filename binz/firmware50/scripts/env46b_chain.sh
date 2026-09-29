#!/bin/bash
# ENV-46b: two more chain captures a side at 37.5% (ABBA), to firm the step-6 late-rate finding.
cd "$(dirname "$0")/.."
A1C=captures/elf/970E2A0D.a1-chain-adv18.elf; TGC=captures/elf/3B99710D.tag-chain-adv18.elf
n=4
for spec in A1:$A1C TG:$TGC TG:$TGC A1:$A1C; do
  n=$((n+1)); side=${spec%%:*}; elf=${spec#*:}
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env46-chain-$n$side 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-46b DONE"
