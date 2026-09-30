#!/bin/bash
# ENV-80: commutation lateness vs rung (chain edge probe + isr-stats twins), same session.
cd "$(dirname "$0")/.."
C725=captures/elf/3D764D73.b4w-adv16-chain.elf; C750=captures/elf/3E4D6A1E.c1-chain750.elf
S725=captures/elf/70231AB8.c1-stats725.elf; S750=captures/elf/7F13D3F5.c1-stats750.elf
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for spec in 700:$C725:1 725:$C725:1 750:$C750:1 700:$C725:2 725:$C725:2 750:$C750:2; do
  R=${spec%%:*}; rest=${spec#*:}; E=${rest%:*}; i=${rest##*:}
  echo "=== chain $R $i"
  python scripts/chain_run.py --elf $E --flash --no-warmup --pre=$(plus $R) --command L --rung-duty $R --timeout 150 --label env80-c$R-$i 2>&1 | grep -E "CHAINSNAP|REFUSED|Error|error:|Traceback" | head -2
done
for spec in 725:$S725 750:$S750; do
  R=${spec%%:*}; E=${spec#*:}
  echo "=== stats $R"
  python scripts/bemf_run.py --elf $E --flash --pre=$(plus $R) --command L --rung-duty $R --runs 1 --timeout 120 --no-ladder --label env80-s$R 2>&1 | grep -E "BEMFSELFREF|REFUSED|Error|Traceback" | head -2
done
echo "### ENV-80 DONE"
