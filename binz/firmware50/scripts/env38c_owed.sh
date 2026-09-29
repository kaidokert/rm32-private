#!/bin/bash
# ENV-38c (owed by the ENV-38/39 review): restarts 3/3 at 600/650/675 and the ADVANCE_LOW walk 150..350, on 7450FE24.
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-38c: ADVANCE_LOW 16->18 re-walk on 7450FE24; only oracle-referenced rungs exist below 375 (175/225 have no reference), so each rung is anchored and judged by its own gates."
plus() { n=$(( ($1 - 400) / 25 )); if [ $n -lt 0 ]; then printf -- '-%.0s' $(seq 1 $(( -n ))); elif [ $n -gt 0 ]; then printf '+%.0s' $(seq 1 $n); fi; }
for spec in 600:xxxx 650:xxxxxx 675:xxxxxxx; do
  R=${spec%%:*}; X=${spec##*:}
  for i in 1 2 3; do
    echo "=== restart $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre $X --command Z --rung-duty $R --runs 1 --timeout 120 --label env38c-rst$R-$i 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|Traceback" | head -3
  done
done
for R in 150 200 250 275 300 325 350; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre="$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env38c-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUN PASS|RUNG|REFUSED|Error|Traceback" | head -4
  done
done
echo "### ENV-38c DONE"
