#!/bin/bash
# ENV-56 (A4) stage 1: fresh 37.5% session. Production ABBAAB (A=tag, B=A4), then chain A T T A A T T A (A=A4).
cd "$(dirname "$0")/.."
TGP=captures/elf/7450FE24.env37-adv18-cap725-qual.elf; A1P=captures/elf/372F207B.a4-sixgate-shell-pwm.elf
A1C=captures/elf/95A0259C.a4-sixgate-chain-capture.elf; TGC=captures/elf/3B99710D.tag-chain-adv18.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$TGP; [ $side = B ] && elf=$A1P
  echo "=== prod $n $side"
  python scripts/bemf_run.py --elf $elf --flash --command J --runs 1 --timeout 120 --no-ladder --label env56-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|REFUSED|Error|error:|Traceback" | head -4
done
n=0
for side in A T T A A T T A; do
  n=$((n+1)); elf=$TGC; [ $side = A ] && elf=$A1C
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env56-chain-$n$side 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-56 STAGE1 DONE"
