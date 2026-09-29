#!/bin/bash
# ENV-59 (A5) stage 1: fresh 37.5% session. Production ABBAAB (A=tag, B=A5), then chain A T T A A T T A (A=A5).
cd "$(dirname "$0")/.."
TGP=captures/elf/7450FE24.env37-adv18-cap725-qual.elf; A1P=captures/elf/B364E7B8.a5-armfirst-shell-pwm.elf
A1C=captures/elf/EB3148A1.a5-armfirst-chain-capture.elf; TGC=captures/elf/3B99710D.tag-chain-adv18.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$TGP; [ $side = B ] && elf=$A1P
  echo "=== prod $n $side"
  python scripts/bemf_run.py --elf $elf --flash --command J --runs 1 --timeout 120 --no-ladder --label env59-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|REFUSED|Error|error:|Traceback" | head -4
done
n=0
for side in A T T A A T T A; do
  n=$((n+1)); elf=$TGC; [ $side = A ] && elf=$A1C
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env59-chain-$n$side 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-59 STAGE1 DONE"
