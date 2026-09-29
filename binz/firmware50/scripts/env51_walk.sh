#!/bin/bash
# ENV-51 (A1r stage 2): walk 525 (anchored) .. 725 on A1 image D7256586 (loadable 9A389DEB), 3 holds per rung;
# stop at the first rung without PASS. Do NOT commit during this walk (ladder_state race).
cd "$(dirname "$0")/.."
ELF=captures/elf/D7256586.a1-adv18-cap725.elf
PROOF="ENV-51 A1r: AM32-placement image D7256586 (loadable 9A389DEB) re-qualifying from 525; production lineage 7450FE24 (72.5% qualified ENV-38); only the comparator/commutation placement differs (ENV-45), stage 1 ENV-50."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for R in 525 550 575 600 625 650 675 700 725; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    if [ $R = 525 ]; then
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env51-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1)
    else
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env51-r$R-$i 2>&1)
    fi
    echo "$out" | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -5
  done
  if ! echo "$out" | grep -q "RUNG .*: PASS"; then echo "### WALK STOPS at $R"; exit 0; fi
done
echo "### WALK DONE"
