#!/bin/bash
# ENV-63 (A6 stage 2) follow-up on 0A978A82: restarts 3/3 at 500/600/650/675/700, 725 on the window-90 twin
# F5F173CF, protection sweep, low rungs 150..500 (each anchored: oracle gate reported separately, Q60-5/ENV-38c).
cd "$(dirname "$0")/.."
ELF=captures/elf/0A978A82.a6-split-shell-pwm.elf; TWIN=captures/elf/235A5DC2.a6-window90.elf
PROOF="ENV-63 A6: low rungs / 500 restart on 0A978A82; oracle coast gate marginal-to-failing on every image below 525 (Q60-5, ENV-38c); judged on non-oracle gates + self-ref."
PROOFW="ENV-63 A6: 725 restart on the window-90 twin 235A5DC2 of 0A978A82 (ISR roots instruction-identical; only BEMF_TOTAL_MS 78->90 s); the 78 s window cannot fit any restart above 700 (ENV-38/41)."
plus() { n=$(( ($1 - 400) / 25 )); if [ $n -lt 0 ]; then printf -- '-%.0s' $(seq 1 $(( -n ))); elif [ $n -gt 0 ]; then printf '+%.0s' $(seq 1 $n); fi; }
for spec in 500:xxx 600:xxxx 650:xxxxxx 675:xxxxxxx 700:xxxxxxxx; do
  R=${spec%%:*}; X=${spec##*:}
  extra=(); [ $R = 500 ] && extra=(--anchor --anchor-proof "$PROOF")
  for i in 1 2 3; do
    echo "=== restart $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre=$X --command Z --rung-duty $R --runs 1 --timeout 120 --label env63-rst$R-$i "${extra[@]}" 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|error:|Traceback" | head -3
  done
done
for i in 1 2 3; do
  echo "=== restart 725 (twin) run $i"
  python scripts/bemf_run.py --elf $TWIN --flash --pre=xxxxxxxxx --command Z --rung-duty 725 --runs 1 --timeout 140 --label env63-rst725w-$i --anchor --anchor-proof "$PROOFW" 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|error:|Traceback" | head -3
done
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder --label env63-prot-$k 2>&1 | grep -E "BEMFINJECT|RESETCAUSE|REFUSED|Error|error:|Traceback" | head -3
done
for spec in b:150 2:200 5:250 A:275 Y:288 C:300 D:325 M:338 E:350; do
  K=${spec%%:*}; R=${spec##*:}
  for i in 1 2 3; do
    echo "=== low $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --command $K --runs 1 --timeout 120 --label env63-k$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "RUN FAIL|REFUSED|Error|error:|Traceback" | head -3
  done
done
for R in 375 400 425 450 475 500; do
  for i in 1 2 3; do
    echo "=== low $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre="$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env63-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "RUN FAIL|REFUSED|Error|error:|Traceback" | head -3
  done
done
echo "### ENV-63 FOLLOWUP DONE"
