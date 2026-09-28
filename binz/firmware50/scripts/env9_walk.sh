#!/bin/bash
# ENV-9: walk 600 (anchored) -> 625 -> 650 on one image, three runs a rung,
# fresh flash per run so the `+` climb is absolute (400 + n x 25).
cd "$(dirname "$0")/.."
ELF=captures/elf/8FFE35E2.env9-adv16-cap650.elf
PROOF="ENV-9: identical to A09BA143 (advance 16, deep-filter, cap 625; 6/6 holds at 625 in ENV-7) except SIXSTEP_DUTY_CAP 625->650, which clamps only above 625, and the x cycle gaining 625->650, a foreground shell constant. isr_diff vs A09BA143: all four roots instruction-identical (ADC_COMP 818, TIM16 368, TIM6 155, DMA1 37); TIM16 differs only in a moved literal-pool constant. isr_audit PASS. Suites 361/361 default, 359/359 advance-ref,deep-filter."
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
run() {  # rung, run index, extra args...
  local r=$1 i=$2; shift 2
  echo "=== r$r run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "$(plus $r)" --command L \
    --rung-duty $r --runs 1 --timeout 120 --label env9-r$r-$i "$@" 2>&1 \
    | grep -E "BEMFSELFREF|RUN PASS|RUN FAIL|RUNG|NOT RECORDED|REFUSED|FAIL|Error|Traceback|KILL|BEMFDONE" | head -10
}
for i in 1 2 3; do run 600 $i --anchor --anchor-proof "$PROOF"; done
for i in 1 2 3; do run 625 $i; done
for i in 1 2 3; do run 650 $i; done
