#!/bin/bash
# ENV-40: three 750 holds on the diagnostic cap-750 image D12EB4BE (advance 18), anchored. Not qualifying.
cd "$(dirname "$0")/.."
ELF=captures/elf/D12EB4BE.env40-adv18-edge750.elf
PROOF="ENV-40 diagnostic: ISR roots instruction-identical to 7450FE24 (walked 525..725, ENV-38); only SIXSTEP_DUTY_CAP 725->750 (edge-probe). Not a qualifying image."
for i in 1 2 3; do
  echo "=== hold 750 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "++++++++++++++" --command L --rung-duty 750 --runs 1 --timeout 120 --label env40-r750-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUN PASS|REFUSED|Error|Traceback" | head -4
done
echo "### ENV-40 DONE"
