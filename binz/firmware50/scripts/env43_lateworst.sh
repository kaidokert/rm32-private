#!/bin/bash
# ENV-43: three 750 holds on the diagnostic cap-750 image D12EB4BE (advance 18), anchored. Not qualifying.
cd "$(dirname "$0")/.."
ELF=captures/elf/46281289.env43-adv18-edge750-lateworst.elf
PROOF="ENV-43 diagnostic: ISR roots instruction-identical to 7450FE24 (walked 525..725, ENV-38); cap 750 (edge-probe) + late-worst reporting. Not a qualifying image."
for i in 1 2 3; do
  echo "=== hold 750 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "++++++++++++++" --command L --rung-duty 750 --runs 1 --timeout 120 --label env43-r750-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUN PASS|REFUSED|Error|Traceback" | head -4
done
echo "### ENV-43 DONE"
