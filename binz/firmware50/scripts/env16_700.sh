#!/bin/bash
# ENV-16: three anchored holds at 700 on the production image, fresh flash per run.
cd "$(dirname "$0")/.."
ELF=captures/elf/530432A2.env16-adv16-cap700.elf
PROOF="ENV-16: identical to B0E5CCD4 (67.5% qualified ENV-15: holds 3/3, restart 3/3, sweep 10/10) except SIXSTEP_DUTY_CAP 675->700 (clamps only above 675), x cycle 675->700, SELF_REF_RUNGS += 700. isr_diff vs B0E5CCD4: four roots instruction-identical, TIM16 one moved pool constant; isr_audit PASS; suites 361/359."
for i in 1 2 3; do
  echo "=== hold 700 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "++++++++++++" --command L --rung-duty 700 \
    --runs 1 --timeout 120 --label env16-r700-$i --anchor --anchor-proof "$PROOF" 2>&1 \
    | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|NOT RECORDED|REFUSED|Error|Traceback|BEMFDONE|FAIL" | head -10
done
