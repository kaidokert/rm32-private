#!/bin/bash
# ENV-63 (A6 stage 2): walk 525 (anchored) .. 725 on A6 image 0A978A82 (loadable 31FE2832), 3 holds per rung;
# stop at the first rung without PASS. Do NOT commit during this walk (ladder_state race).
cd "$(dirname "$0")/.."
ELF=captures/elf/0A978A82.a6-split-shell-pwm.elf
PROOF="ENV-63 A6: AM32-split image 0A978A82 (loadable 31FE2832) re-qualifying from 525; production lineage 7450FE24 (72.5% qualified ENV-38); only the comparator/commutation placement differs (ENV-45), stage 1 ENV-62."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for R in 525 550 575 600 625 650 675 700 725; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    if [ $R = 525 ]; then
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env63-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1)
    else
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env63-r$R-$i 2>&1)
    fi
    echo "$out" | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -5
  done
  if ! echo "$out" | grep -q "RUNG .*: PASS"; then echo "### WALK STOPS at $R"; exit 0; fi
done
echo "### WALK DONE"
