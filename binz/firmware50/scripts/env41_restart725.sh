#!/bin/bash
# ENV-41: restart at 725 on the labelled 90 s-window diagnostic image 74346ED6 (diagnostic, not qualifying).
cd "$(dirname "$0")/.."
ELF=captures/elf/74346ED6.env41-adv18-cap725-window90.elf
PROOF="ENV-41 diagnostic: ISR roots instruction-identical to 7450FE24 (walked 525..725 in ENV-38); only BEMF_TOTAL_MS 78->90 s (window-90). Not qualifying."
for i in 1 2 3; do
  echo "=== restart 725 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre xxxxxxxxx --command Z --rung-duty 725 --runs 1 --timeout 140 --label env41-rst725-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFRESTARTRUN|BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|error:|Traceback" | head -4
done
echo "### ENV-41 DONE"
