# E723 — practical startup current and waveform checks

The goal does not specify500mA. The original nominal average allowance was an
agent-selected number. A small cfg-only diagnostic now retains the same limit's
zero, allowance, last completed50-scan residual, refusal class and fault raw.
It prints after shutdown only, remains separate from qualification images, and
does not change the protection decision. No per-sample recorder was added.

| Image / capture | Result before live throttle |
|---|---|
| avgdiag_723 / avgdiag_723_direct200.txt | Startup average refusal, residual2369 >1991, nonrail cause3, 4200 scans,0.844503s |
| avg800_723b / avg800_723b_direct300.txt | Startup average refusal4834 >3185, 5150 scans,1.035407s |
| start20k_723c / start20k_723c_direct300.txt | No average fault,23447 scans; driven stop20 at12.784ms because comparator recorder filled |
| nopcmp_723d / nopcmp_723d_direct300.txt | No average fault,23485 scans; driven stop2 at20.182ms, seed missing-epoch fault2 |

The nominal average target is now800mA, tied to the operator's existing PSU
ceiling, not raised iteratively to clear observed residuals. Its3185 raw-sum
allowance retains assumed3.6V, gain10, shunt7mohm,50scans. These assumptions
are explicit, not measured calibration or a certified800mA bound. Timed
exploration now accepts nonrailed pulse amplitudes instead of treating the old
848..3248 cutoff as a calibrated SOA clamp. Average-current, rails, bus sag,
nFAULT, tracking and bounded execution protection remain. Pulse amplitudes
remain available in raw captures; production ratings-based clamp is unfinished.

Source inspection found a concrete restart defect: `prepare_sine()` restored
PWM modes but not ARR after BEMF changed the carrier. It now writes and verifies
the sine ARR before UG loads preloads. The new `bench-startup-20k` selects20kHz
sine PWM instead of the original10kHz; both compare and reported ON-time scales
derive from the same compile-time carrier constant. The current limit was not
raised after723b. One20k startup passed current protection; this small sequence
with different baseline wakes is not a quantified current/carrier A/B study.

At20kHz the256sample comparator DMA microscope filled in12.8ms and stopped
the20ms handoff campaign. It has no handoff authority. LeanIRQ now omits its
prepare/start/healthy-buffer-capacity stop; default diagnostic paths retain it.
The next attempt reached the20ms acquisition deadline instead, with18 accepted
events across24 commanded boundaries, handlermax25us and zero recorded
overruns. Decoded DS85 `(0,0,0,0,0,2,0,0,0,2)` identifies no ready seed, only
two fresh intervals and missing-epoch fault2. This is initial under-drive seed
failure, not flying recovery and not demonstrated loss of established lock.

All four are failed exploratory starts: no higher-duty ACK and no BEMF handoff.
Host final-off readbacks verify gates/ENABLE/MOE/CCRs off; UART sessions closed.
All release-s/thinLTO builds and TIM16 arithmetic-helper gates passed. The25
pure startup/current/cache/clock tests passed after nominal800 selection; this
does not establish complete feature-specific ISR timing or physical calibration.

Installed ELF `captures/reference/nopcmp_723d/shell-pwm.elf` SHA256:
`5ef1f7f665cb1647f47de186ecef86cfb17ea943befa59d572ac9c8bcf13c257`.
Predecessor image hashes:
`588dcb0fb360f994ce3a8dbf289dd6370eea3082fea7da18ae8efdbe78c26f4b`,
`dd6701adc28d81ae6e2a93ad69e7e2702f238e6fc767b8ccfe5d40c91ec78eee`,
`8adc8b9677af39af525f7412afffed461597fce1a273ec1e7192bc6ac6e38b38`.

Next useful work is normal startup/under-drive handoff using reference-like
mode/seed behavior. No further nominal-current threshold increase is planned;
flying-recovery cohorts remain outside the exploration gate.
