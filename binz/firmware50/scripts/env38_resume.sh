#!/bin/bash
# ENV-37: re-qualify every earned rung (525..700) and qualify 725 on the advance-18 image
# 7450FE24 (loadable CB638C58 = the ENV-36 B arm). 3 holds per rung; stop at the first failed rung.
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-37: advance 18 (ENV-36 lever) on the qualified lineage E1256E38 (advance 16, cap 700, 70% qualified ENV-23); identical code otherwise, cap 725 as the step under qualification. Loadable CB638C58 is byte-identical to the ENV-36 B arm C567066B, which held 725 3/3 with 0 foldback (worst 4305-4419). ISR roots instruction-identical to E1256E38 (advance is foreground); isr_audit PASS; suites 364/362."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for R in 550 575 600 625 650 675 700 725; do
  for i in $( [ $R = 550 ] && echo 4 || echo 1 2 3 ); do
    echo "=== hold $R run $i"
    if [ $R = 525 ]; then
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env37-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1)
    else
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env37-r$R-$i 2>&1)
    fi
    echo "$out" | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -5
  done
  if ! echo "$out" | grep -q "RUNG .*: PASS"; then echo "### WALK STOPS at $R"; exit 0; fi
done
echo "### WALK DONE"
