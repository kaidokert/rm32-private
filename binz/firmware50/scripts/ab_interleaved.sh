#!/bin/bash
# Matched A/B, ABABAB in one session, fresh flash per run so the `+` climb is
# absolute (climb_tenths resets to 400; each `+` is 25). Exploratory: --no-ladder.
#   bash scripts/ab_interleaved.sh <elfA> <elfB> <rung> <label>
# ENV-11: bash scripts/ab_interleaved.sh captures/elf/8FFE35E2.env9-adv16-cap650.elf \
#           captures/elf/FF036E17.env10-adv18-cap650.elf 650 env11
cd "$(dirname "$0")/.."
A=${1:?elf A}; B=${2:?elf B}; R=${3:?rung}; LABEL=${4:?label}
PLUS=$(printf '+%.0s' $(seq 1 $(( (R - 400) / 25 ))))
for i in 1 2 3; do
  for side in A B; do
    elf=$A; [ $side = B ] && elf=$B
    echo "=== $side$i $elf"
    python scripts/bemf_run.py --elf $elf --flash --pre "$PLUS" --command L \
      --rung-duty $R --runs 1 --timeout 120 --no-ladder --label $LABEL-$side$i 2>&1 \
      | grep -E "BEMFRUN|BEMFDONE|BEMFSELFREF|REFUSED|FAIL|Error|Traceback" | head -8
  done
done
