#!/bin/bash
# ENV-17 control: 675 x3 on the qualified image B0E5CCD4 on the 5 A supply (re-run of
# earned; tests whether the 700 sag-margin jump is the PSU change). Fresh flash per run.
cd "$(dirname "$0")/.."
A=captures/elf/B0E5CCD4.env12-adv16-cap675.elf
PROOF="ENV-17 re-run of earned 675 on B0E5CCD4 after the operator raised the PSU limit 3 A -> 5 A (hardware change). Same image that qualified 675 in ENV-13..15 (anchored there on 8FFE35E2, ISR-instruction-identical)."
for i in 1 2 3; do
  echo "=== A 675 prod run $i"
  python scripts/bemf_run.py --elf $A --flash --pre "+++++++++++" --command L --rung-duty 675 \
    --runs 1 --timeout 120 --label env17-ctl675-$i --anchor --anchor-proof "$PROOF" 2>&1 \
    | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -6
done
