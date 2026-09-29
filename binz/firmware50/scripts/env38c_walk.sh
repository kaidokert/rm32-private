#!/bin/bash
# ENV-38c: the ADVANCE_LOW (<35%) walk on 7450FE24 via the firmware's rung keys (the shell's -/+ climb floors at 375).
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-38c: ADVANCE_LOW 16->18 re-walk on 7450FE24; the walk starts at the lowest rung key (b, 15%)."
for spec in b:150 2:200 5:250 A:275 Y:288 C:300 D:325 M:338 E:350; do
  K=${spec%%:*}; R=${spec##*:}
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    extra=(); [ $R = 150 ] && extra=(--anchor --anchor-proof "$PROOF")
    python scripts/bemf_run.py --elf $ELF --flash --command $K --runs 1 --timeout 120 --label env38c-k$R-$i "${extra[@]}" 2>&1 | grep -E "RUN FAIL|RUN PASS|RUNG|REFUSED|Error|error:|Traceback" | head -4
  done
done
echo "### ENV-38c WALK DONE"
