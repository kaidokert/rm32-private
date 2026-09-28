#!/bin/bash
# ENV-9: one image, rung A vs rung B, ABABAB in one session, fresh flash per run
# so the `+` climb is absolute (climb_tenths resets to 400; each `+` is 25).
#   bash scripts/env9_rungs.sh captures/elf/8FFE35E2.env9-adv16-cap650.elf 625 650 env9
cd "$(dirname "$0")/.."
ELF=${1:?elf}; RA=${2:?rung A}; RB=${3:?rung B}; LABEL=${4:?label}
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for i in 1 2 3; do
  for r in $RA $RB; do
    echo "=== r$r run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $r)" --command L \
      --rung-duty $r --runs 1 --timeout 120 --label $LABEL-r$r-$i 2>&1 \
      | grep -E "BEMFRUN|BEMFDONE|BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|NOT RECORDED|REFUSED|FAIL|Error|Traceback|KILL" | head -10
  done
done
