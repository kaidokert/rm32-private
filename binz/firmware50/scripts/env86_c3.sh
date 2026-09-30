#!/bin/bash
# ENV-86: C3 anchored qualification (FC76E4B6), restarts at 750 on 171BB26F, sweep, low spot checks.
cd "$(dirname "$0")/.."
ELF=captures/elf/FC76E4B6.c3-prod.elf; TWIN=captures/elf/171BB26F.c3-window90.elf
PROOF="ENV-86 C3 FC76E4B6 (loadable 7B4766B6): all four ISRs instruction-identical to C2 882839DC, which passed the full ENV-63 kit to 750 (ENV-82); C3 differs only in foreground/init (storm budget reset at arm, CHSELRMOD/CCRDY ordering + readback)."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for R in 525 650 750; do
  for i in 1 2 3; do
    echo "=== anchor $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $R)" --command L --rung-duty $R --runs 1 --timeout 120 --label env86-r$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -4
  done
done
for i in 1 2 3; do
  echo "=== restart 750 (twin) run $i"
  python scripts/bemf_run.py --elf $TWIN --flash --pre=xxxxxxxxxx --command Z --rung-duty 750 --runs 1 --timeout 140 --label env86-rst750w-$i --anchor --anchor-proof "$PROOF (window-90 twin 171BB26F)" 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|Traceback" | head -3
done
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder --label env86-prot-$k 2>&1 | grep -E "BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -3
done
for spec in b:150 C:300; do
  K=${spec%%:*}; R=${spec##*:}
  for i in 1 2 3; do
    echo "=== low $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --command $K --runs 1 --timeout 120 --label env86-k$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "RUN FAIL|REFUSED|Error|Traceback" | head -3
  done
done
for i in 1 2 3; do
  echo "=== low 500 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre="$(plus 500)" --command L --rung-duty 500 --runs 1 --timeout 120 --label env86-r500-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "RUN FAIL|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-86 DONE"
