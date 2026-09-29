#!/bin/bash
# ENV-38b: low rungs 400..500 and restart 500 on 7450FE24, every rung anchored.
# Rungs <=500 cannot pass the host oracle-coast gate on this motor on ANY image (Q60-3:
# ORACLE was measured on the previous motor; advance 16 B1E51E59 fails it at 375 and 500).
# Each run is judged on every other gate + the BEMFSELFREF within-run identity, by env38_low_check.py.
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-38b: rungs <=500 fail only the previous-motor oracle coast gate on every image (Q60-3; adv16 B1E51E59 375=1551, 500=1912-1921); judged on all other gates + BEMFSELFREF identity."
plus() { n=$(( ($1 - 400) / 25 )); if [ $n -lt 0 ]; then printf -- '-%.0s' $(seq 1 $(( -n ))); elif [ $n -gt 0 ]; then printf '+%.0s' $(seq 1 $n); fi; }
for R in 400 425 450 475 500; do
  for i in $( [ $R = 400 ] && echo 3 4 5 || echo 1 2 3 ); do
    echo "=== hold $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env38b-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -4
  done
done
for i in 1 2 3; do
  echo "=== restart 500 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre xxx --command Z --rung-duty 500 --runs 1 --timeout 120 --label env38b-rst500-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-38b DONE"
