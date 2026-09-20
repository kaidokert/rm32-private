# E489 — long/short redistribution occurs in passing captures too

Offline only; actual C631 firmware and last verified-off state from E488 are
unchanged. No flash, UART or motor run in this entry.

`scripts/drv_boundary_pairs.py` validates each complete capture through the
existing summary/CRC/off verifier, then ranks adjacent reference-counter
intervals in its contiguous 32-event tail. Each interval is compared with the
median of other retained intervals ending at the same sector, excluding both
candidate rows. At least two other observations per sector are required.
The ranking score is `max(0, min(long_delta, -short_delta))`; it is a descriptive
balanced-pair amplitude, NOT a detector threshold or measured physical delay.
Identity is the zero-based accepted count before the first boundary.

Largest ranked pair in each explicitly selected capture:

| Capture basename (in captures/, .txt) | Outcome | Identity / step | Long delta us | Next delta us |
|---|---|---|---:|---:|
| carrier20_486_start61_hold70_10s_01 | Failed | 393 / 2 | +90.75 | -67.00 |
| range350_start61_hold73_10s_01 | Failed | 380 / 3 | +72.00 | -62.50 |
| carrier20_485_start61_reentry69_30s_01 | Passed | 56112 / 2 | +34.50 | -41.50 |
| carrier20_485_start61_reentry69_30s_02 | Passed | 56018 / 5 | +56.25 | -64.25 |
| carrier20_485_start61_reentry69_30s_03 | Passed | 56065 / 1 | +64.75 | -41.25 |
| seedmask_488_start61_reentry69_30s_01 | Passed | 56053 / 1 | +28.50 | -34.75 |

The script emits each raw capture SHA256 for reproducibility. All six pass
their existing CRC/chronology/final-off checks. Four unit tests cover synthetic
displacement and identity, fixed sector asymmetry, discontinuity/invalid data,
insufficient baseline, and the retained passing/failing examples.

This extends E479: the strongest pair in E486 is again the previous visit to
the later-refused sector (399 accepted total minus6 =393). However, a sizable
opposite-sign pair is also present in an ordinary passing tail. Its existence
alone does not distinguish a fault. The two failed samples are acceleration
tails; the passing ones are end-of-recovery tails, with different speeds and
some different builds. Their maxima cannot establish a safe threshold,
population frequency, carrier quantization, preemption or physical overspeed.
All unrecorded middle events remain unobserved. Do not interpolate them.

Next diagnostic requirement: associate gate/reject/accept history with each
accepted boundary over at least two electrical cycles, including normal
boundaries, while retaining the static comparator read path. A compact
per-accepted summary could amortize publication compared with the rejected
all-dispatch recorders, but per-call accumulation still has real cost and
needs its own disabled timing budget. It must distinguish missing history
from zero rejections and preserve epoch/reset/stop provenance. No such new
firmware instrument is installed or qualified by this offline study.

Do not use this result to broaden IRQ masking, alter the reused filter, change
hardware, or relax the running-cycle guard. Current calibration and the
independent physical-edge timing gap remain open.
