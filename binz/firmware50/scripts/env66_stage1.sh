#!/bin/bash
# ENV-66 (B1) stage 1: fresh 37.5% session. Production ABBAAB (A=A6, B=B1), then chain A T T A A T T A (A=B1).
cd "$(dirname "$0")/.."
TGP=captures/elf/0A978A82.a6-split-shell-pwm.elf; A1P=captures/elf/F6F97A08.b1-nostats-shell-pwm.elf
A1C=captures/elf/A3C6B328.b1-nostats-chain-capture.elf; TGC=captures/elf/0440C289.a6-split-chain-capture.elf
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$TGP; [ $side = B ] && elf=$A1P
  echo "=== prod $n $side"
  python scripts/bemf_run.py --elf $elf --flash --command J --runs 1 --timeout 120 --no-ladder --label env66-prod-$n$side 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|REFUSED|Error|error:|Traceback" | head -4
done
n=0
for side in A T T A A T T A A T T A A T T A; do
  n=$((n+1)); elf=$TGC; [ $side = A ] && elf=$A1C
  echo "=== chain $n $side"
  python scripts/chain_run.py --elf $elf --flash --no-warmup --command J --timeout 130 --label env66-chain-$n$side 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -3
done
echo "### ENV-66 STAGE1 DONE"
