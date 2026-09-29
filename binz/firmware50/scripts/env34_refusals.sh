#!/bin/bash
cd "$(dirname "$0")/.."
ELF=captures/elf/A2C3FF31.env34-chain-refusals3-edge725.elf
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for spec in 725:1 700:1 725:2 700:2 725:3; do
  R=${spec%%:*}; i=${spec##*:}
  echo "=== chain $R run $i"
  python scripts/chain_run.py --elf $ELF --flash --no-warmup --pre "$(plus $R)" --command L --rung-duty $R \
    --timeout 150 --label env34-chain$R-$i 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-34 DONE"
