#!/bin/bash
# Matched A/B in ABBAAB order, one session, fresh flash per run (absolute `+` climb).
#   bash scripts/ab_abbaab.sh <elfA> <elfB> <rung> <label>
cd "$(dirname "$0")/.."
A=${1:?elf A}; B=${2:?elf B}; R=${3:?rung}; LABEL=${4:?label}
PLUS=$(printf '+%.0s' $(seq 1 $(( (R - 400) / 25 ))))
n=0
for side in A B B A A B; do
  n=$((n+1)); elf=$A; [ $side = B ] && elf=$B
  echo "=== $n $side $elf"
  python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L --rung-duty $R \
    --runs 1 --timeout 120 --no-ladder --label $LABEL-$n$side 2>&1 \
    | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -6
done
