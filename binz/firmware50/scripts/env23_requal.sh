#!/bin/bash
# ENV-23: (a) control rings at 700 on sag-capture 577A53B0 (unfrozen tails: the steady-state
# per-sector current to compare with the frozen 725 surge rings); (b) re-qualify 70 % on the
# corrected production image E1256E38 (cap back to 700): 3 anchored holds, 3 restarts, sweep.
cd "$(dirname "$0")/.."
SC=captures/elf/577A53B0.env23-sagcapture-cap700-5A.elf
ELF=captures/elf/E1256E38.env23-adv16-cap700-5A.elf
PROOF="ENV-23: identical to B734ACCD (70% qualified ENV-21: holds 3/3, restart 3/3, sweep 10/10, 5 A allowance) except SIXSTEP_DUTY_CAP 800 -> 700 (the qualified top; closes the sag-stimulus over-cap path the ENV-20/21 review found) and an observer-only freeze in the sag ring (production byte-identical). isr_diff vs B734ACCD: four roots instruction-identical; audit PASS; suites 361/359/359/359."
for i in 1 2 3; do
  echo "=== control ring 700 run $i"
  python scripts/sag_run.py --elf $SC --flash --no-warmup --pre "++++++++++++" --command L --rung-duty 700 \
    --timeout 150 --label env23-ctl700-$i 2>&1 | grep -E "BEMFSELFREF|SAGSNAP|REFUSED|Error|Traceback" | head -4
done
for i in 1 2 3; do
  echo "=== hold 700 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre "++++++++++++" --command L --rung-duty 700 --runs 1 \
    --timeout 120 --label env23-r700-$i --anchor --anchor-proof "$PROOF" 2>&1 | grep -E "BEMFSELFREF|RUN FAIL|RUNG|REFUSED|Error|Traceback" | head -5
done
for i in 1 2 3; do
  echo "=== restart 700 run $i"
  python scripts/bemf_run.py --elf $ELF --flash --pre xxxxxxxx --command Z --rung-duty 700 --runs 1 \
    --timeout 120 --label env23-rst700-$i 2>&1 | grep -E "BEMFRESTART |RESTART (PASS|FAIL)|REFUSED|Error|Traceback" | head -4
done
for k in t g f n u h i k q w; do
  echo "=== key $k"
  python scripts/bemf_run.py --elf $ELF --flash --command $k --runs 1 --timeout 120 --no-ladder \
    --label env23-prot-$k 2>&1 | grep -E "BEMFINJECT|RESETCAUSE|REFUSED|Error|Traceback" | head -3
done
echo "### ENV-23 DONE"
