#!/bin/bash
# ENV-38 follow-up on 7450FE24: low rungs 375..500 (375 anchored), restarts 3/3 at 500, 700, 725, protection sweep.
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-38 low-rung re-qualification of advance 18 (ADVANCE_LOW 18 changes the ramp below 35%); same image walked 525..725 in ENV-38."
plus() { n=$(( ($1 - 400) / 25 )); if [ $n -lt 0 ]; then printf -- '-%.0s' $(seq 1 $(( -n ))); elif [ $n -gt 0 ]; then printf '+%.0s' $(seq 1 $n); fi; }
for R in 375 400 425 450 475 500; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    extra=(); [ $R = 375 ] && extra=(--anchor --anchor-proof "$PROOF")
    python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env38-r$R-$i "${extra[@]}" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -4
  done
done
for spec in 500:xxx 700:xxxxxxxx 725:xxxxxxxxx; do
  R=${spec%%:*}; X=${spec##*:}
  for i in 1 2 3; do
    echo "=== restart $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre $X --command Z --rung-duty $R --runs 1 --timeout 120 --label env38-rst$R-$i 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|Traceback" | head -3
  done
done
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder --label env38-prot-$k 2>&1 | grep -E "BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-38 FOLLOWUP DONE"
