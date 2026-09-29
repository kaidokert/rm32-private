#!/bin/bash
# ENV-46: A1 steps 2-3 at 37.5% (rung J). Chain ABBA, then production ABBAAB.
cd "$(dirname "$0")/.."
A1C=captures/elf/970E2A0D.a1-chain-adv18.elf; TGC=captures/elf/3B99710D.tag-chain-adv18.elf
A1P=captures/elf/D7256586.a1-adv18-cap725.elf; TGP=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
n=0
for spec in A1:$A1C TG:$TGC TG:$TGC A1:$A1C; do
  n=$((n+1)); side=${spec%%:*}; elf=${spec#*:}
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env46-chain-$n$side 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|saved|REFUSED|Error|error:|Traceback" | head -5
done
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$TGP; [ $side = B ] && elf=$A1P
  echo "=== prod $n $side"
  python scripts/bemf_run.py --elf $elf --flash --command J --runs 1 --timeout 120 --no-ladder --label env46-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUN PASS|REFUSED|Error|error:|Traceback" | head -4
done
echo "### ENV-46 DONE"
