#!/bin/bash
cd "$(dirname "$0")/.."
ELF=captures/elf/2B8B8338.env28-edgecapture-f5-edge725.elf
plus() { printf '+%.0s' $(seq 1 $(( ($1 - 400) / 25 ))); }
for spec in 725:1 700:1 725:2; do
  R=${spec%%:*}; i=${spec##*:}
  echo "=== edges $R run $i"
  python scripts/edge_run.py --elf $ELF --flash --no-warmup --pre "$(plus $R)" --command L --rung-duty $R \
    --timeout 150 --label env28-edges$R-$i 2>&1 | grep -E "BEMFSELFREF|CAPSNAP|decisions|REFUSED|Error|Traceback" | head -4
done
echo "### ENV-28 DONE"
