#!/bin/bash
# ENV-25: commutation chain frozen at the first over-block. 725 x3, then 700 x2 (control).
cd "$(dirname "$0")/.."
ELF=captures/elf/EB409F01.env25-chaincapture-edge725-freeze.elf
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for spec in 725:1 700:1 725:2 700:2 725:3; do
  R=${spec%%:*}; i=${spec##*:}
  echo "=== chain $R run $i"
  python scripts/chain_run.py --elf $ELF --flash --no-warmup --pre "$(plus $R)" --command L --rung-duty $R \
    --timeout 150 --label env25-chain$R-$i 2>&1 | grep -E "BEMFSELFREF|CHAINSNAP|saved|REFUSED|Error|Traceback" | head -5
done
echo "### ENV-25 DONE"
