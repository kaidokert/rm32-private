#!/bin/bash
# ENV-13: qualify 67.5 % on B0E5CCD4 -- three holds anchored at 675 (65 % earned on
# 8FFE35E2, ISR-instruction-identical), then restart 3/3 at 675. Fresh flash per run.
cd "$(dirname "$0")/.."
ELF=captures/elf/B0E5CCD4.env12-adv16-cap675.elf
PROOF="ENV-13: identical to 8FFE35E2 (advance 16, deep-filter; 65% earned: 600/625/650 walked 3/3, restart 3/3 at 650, sweep, ENV-9/10) except SIXSTEP_DUTY_CAP 650->675 (clamps only above 650), x cycle 650->675 and SELF_REF_RUNGS += 675. isr_diff vs 8FFE35E2: four roots instruction-identical, TIM16 one moved pool constant; isr_audit PASS; suites 361/359. Exploratory 675 hold env13-probe675 passed: 2362 eHz, operator-metered 2.83 A, worst 3670, sag 16.3 codes."
for i in 1 2 3; do
  echo "=== hold 675 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "+++++++++++" --command L --rung-duty 675 \
    --runs 1 --timeout 120 --label env13-r675-$i --anchor --anchor-proof "$PROOF" 2>&1 \
    | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|NOT RECORDED|REFUSED|Error|Traceback|BEMFDONE" | head -8
done
for i in 1 2 3; do
  echo "=== restart 675 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre xxxxxxx --command Z --rung-duty 675 \
    --runs 1 --timeout 120 --label env13-rst675-$i 2>&1 \
    | grep -E "BEMFRESTARTRUN|BEMFRESTART |RESTART (PASS|FAIL)|RUNG|NOT RECORDED|REFUSED|Error|Traceback" | head -8
done
