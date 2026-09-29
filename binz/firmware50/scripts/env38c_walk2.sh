#!/bin/bash
# ENV-38c (cont.): 200..350 on 7450FE24 via rung keys, each rung anchored: the oracle coast gate is marginal at these
# rungs on BOTH advances (150: adv16 q60-anchor150 = 669, adv18 = 667-669, floor 668.8), so a walked ladder would stop
# on an oracle coin-flip. Judged afterwards on every gate, the oracle reported separately.
cd "$(dirname "$0")/.."
ELF=captures/elf/7450FE24.env37-adv18-cap725-qual.elf
PROOF="ENV-38c: ADVANCE_LOW 16->18 re-walk on 7450FE24; oracle coast gate marginal on both advances at low rungs (150: adv16 669, adv18 667-669 vs floor 668.8); each rung anchored, oracle reported separately."
for spec in 2:200 5:250 A:275 Y:288 C:300 D:325 M:338 E:350; do
  K=${spec%%:*}; R=${spec##*:}
  for i in 1 2 3; do
    echo "=== hold $R run $i"
    python scripts/bemf_run.py --elf $ELF --flash --command $K --runs 1 --timeout 120 --label env38c-k$R-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "RUN FAIL|RUN PASS|RUNG|REFUSED|Error|error:|Traceback" | head -4
  done
done
echo "### ENV-38c WALK DONE"
