"""Synthetic end-to-end test for the ZC analysis pipeline.

Run from the rinz root: python scripts/zc_selftest.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from scope_common import parse_capture, plot_zc_snapshot, format_zc_report

SAMPLE_HZ = 20000
HZ = 50
FPS = SAMPLE_HZ / (HZ * 6.0)
FRAMES = int(round(FPS * 12))  # 2 electrical revs

SIX_STEP_HIGH = [0, 0, 1, 1, 2, 2]
SIX_STEP_LOW = [1, 2, 2, 0, 0, 1]
HI_V = 2000
LO_V = 50
RAMP_LO = 500
RAMP_HI = 1500

# float-phase slope per sector (drive sequence: prev hi -> float falls, etc.)
RAMP_RISING = [False, True, False, True, False, True]

rows = []
for i in range(FRAMES):
    sector = int(i / FPS) % 6
    pos = (i / FPS) % 1.0
    hi = SIX_STEP_HIGH[sector]
    lo = SIX_STEP_LOW[sector]
    fl = 3 - hi - lo
    v = [0, 0, 0]
    v[hi] = HI_V
    v[lo] = LO_V
    if RAMP_RISING[sector]:
        v[fl] = int(RAMP_LO + (RAMP_HI - RAMP_LO) * pos)
    else:
        v[fl] = int(RAMP_HI - (RAMP_HI - RAMP_LO) * pos)
    rows.append("{:04x} {:04x} {:04x}".format(v[0], v[1], v[2]))

text = (
    "capture: wait zero, 2 electrical revs\r\n"
    f"debug: mode=six-step hz={HZ} amp=300 tim7=800 sine_ticks=0 six_ticks=800 dma_tc=1 dma_ht=1 dma_te=0\r\n"
    "regs: t1_cr2=00000070 t1_arr=109a t1_ccr4=0001\r\n"
    f"dump3: {FRAMES} frames x 3 channels (ch17 ch5 ch14, 12-bit ADC, {SAMPLE_HZ} Hz)\r\n"
    + "\r\n".join(rows)
    + "\r\n\rend\r\n"
)

cap = parse_capture(text)
print("frames", cap.frames, "full_scale", cap.full_scale, "sample_hz", cap.sample_hz)
sectors = plot_zc_snapshot(cap, Path("logs/zc_selftest.png"))
print(format_zc_report(sectors))

# Ideal crossing: float = mean-of-three => f = (HI_V + LO_V) / 2 = 1025
# Rising ramp 500->1500 crosses at 52.5%; falling 1500->500 at 47.5%.
ok = all(
    s.status == "zc"
    and abs(s.zc_pct - (52.5 if RAMP_RISING[s.index % 6] else 47.5)) < 4.0
    for s in sectors
)
print("PASS" if ok else "FAIL")
