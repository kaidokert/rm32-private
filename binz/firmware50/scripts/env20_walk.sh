#!/bin/bash
# ENV-20: ladder walk 700 -> 800 on the 5 A allowance image, stopping at the first rung
# that does not pass. Per rung: 3 holds (700 anchored), 3 restarts, 1 margin-hist run.
# Fresh flash per run so every relative key sequence is absolute.
cd "$(dirname "$0")/.."
ELF=captures/elf/B734ACCD.env20-adv16-cap800-5A.elf
MH=captures/elf/9E5BED16.env20-adv16-cap800-5A-mhist.elf
PROOF="ENV-20: identical to 530432A2 (advance 16, deep-filter, 67.5% qualified lineage B0E5CCD4) except (a) AverageCurrent allowance 4000 -> 5000 mA by OPERATOR DECISION (PSU 5 A), (b) mean/window mA scaled by the calibration, (c) cap 700 -> 800 as a ladder ceiling, x cycle and SELF_REF_RUNGS to 800. isr_diff vs 530432A2: four roots instruction-identical; audit PASS; suites 361/359/359."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
xs()   { case $1 in 700) echo xxxxxxxx;; 725) echo xxxxxxxxx;; 750) echo xxxxxxxxxx;; 775) echo xxxxxxxxxxx;; 800) echo xxxxxxxxxxxx;; esac; }
for R in 700 725 750 775 800; do
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    extra=""; [ $R = 700 ] && extra="--anchor"
    if [ -n "$extra" ]; then
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env20-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1)
    else
      out=$(python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env20-r$R-$i 2>&1)
    fi
    echo "$out" | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|REFUSED|Error|Traceback|BEMFDONE" | head -8
  done
  if ! echo "$out" | grep -q "RUNG .*: PASS"; then echo "### WALK STOPS at $R"; break; fi
  for i in 1 2 3; do
    echo "=== restart $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre "$(xs $R)" --command Z --rung-duty $R --runs 1 --timeout 120 --label env20-rst$R-$i 2>&1 \
      | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|RUNG|REFUSED|Error|Traceback" | head -6
  done
  echo "=== mhist $R"
  python scripts/bemf_run.py --elf $MH --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --no-ladder --label env20-mh$R 2>&1 \
    | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -4
done
echo "### WALK DONE"
