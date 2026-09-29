#!/bin/bash
# ENV-31: step-3 late-accept rate vs duty (650, 675), chain image BE2213DE, fresh flash per run.
cd "$(dirname "$0")/.."
ELF=captures/elf/BE2213DE.env26-chain-origin-refusals-edge725.elf
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for spec in 650:1 675:1 650:2 675:2; do
  R=${spec%%:*}; i=${spec##*:}
  echo "=== chain $R run $i"
  python scripts/chain_run.py --elf $ELF --flash --no-warmup --pre "$(plus $R)" --command L --rung-duty $R \
    --timeout 150 --label env31-chain$R-$i 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-31 DONE"
