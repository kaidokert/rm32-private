#!/bin/bash
# ENV-63 (A6 stage 2): walk 525 (anchored) .. 725 on A6 image 0A978A82 (loadable 31FE2832), 3 holds per rung;
# stop at the first rung without PASS. Do NOT commit during this walk (ladder_state race).
cd "$(dirname "$0")/.."
ELF=captures/elf/882839DC.c2-prod.elf
PROOF="ENV-82 C2: dual-shunt estimator + cap 750 on the AM32 detector, image 882839DC (loadable 7C8CF1E3) re-qualifying from 525; base fw50-am32-a6 (0A978A82, 72.5% ENV-63); B4 replaces the detector (control-path change, full re-walk), stage 1 ENV-71, advance A/B ENV-72."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for R in 525 550 575 600 625 650 675 700 725 750; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    if [ $R = 525 ]; then
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env82-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1)
    else
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env82-r$R-$i 2>&1)
    fi
    echo "$out" | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -5
  done
  if ! echo "$out" | grep -q "RUNG .*: PASS"; then echo "### WALK STOPS at $R"; exit 0; fi
done
echo "### WALK DONE"
