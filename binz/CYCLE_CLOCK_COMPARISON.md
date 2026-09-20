# Cycle refusal clocks - E405, 2026-09-14

Offline analysis only; installed044F and verified-off state from E404 unchanged.
The previous turn was progress: powered exclusion falsified its sufficiency as
a fix. No new motor attempt or firmware/guard change in this analysis.

## What the two measurements actually measure

`powered_timer::accepted` samples its clock immediately before RunGuard accepts
the EV_ACC callback. The cycle guard compares successive samples of the same
sector. This is accepted-event delivery timing, not a physical rotor sensor.

`../minz/core/src/am32_isr.rs` samples the interval timer AFTER persistence,
then resets it and arms COM, before calling EV_ACC. Six adjacent reference
intervals therefore cover the corresponding accepted-sector span with reset
gaps and endpoint differences. They are not an independent rotor measurement.

The offline report now exposes their arithmetic closure explicitly, without
calling it ISR latency. Existing CRC/chronology/finaloff validation remains.

| Retained 6.9% failure | Guard cycle us | Reference sum us | Difference us |
|---|---:|---:|---:|
| range330_hold69_01 | 3009 | 3006 | 3 |
| binmath_reentry69_30s_01 | 3012 | 3009 | 3 |
| dmapeer_reentry69_30s_01 | 3011 | 3009 | 2 |
| compcritical_reentry69_30s_01 | 3029 | 3026.5 | 2.5 |

The short accepted-event cycle exists upstream of the guard/recorder callback
in all four examples. A large callback-only delay is not needed to explain
these trips. This does NOT exclude delayed dispatch before the reference
timer sample, rejected qualification, PWM-phase dependence, timer-reset gaps,
or real rotor variation. It also does not make the guard wrong.

## Do not use threshold-selected failures as a quantization histogram

These examples are selected because a cycle crossed the same3031us floor.
Their clustering is consequently conditioned by a stopping rule; they cannot
establish n-carrier-period quantization or its absence. Adjacent long/short
cycles are consistent with crossing redistribution, not proof of a preemptor.
The retained tail is also conditioned on proximity to the fault, and only
contains accepted events, not all rejected physical/comparator edges.

A useful next discriminator needs uncensored accepted-cycle timing across
passing windows AND the acceptance path's rejected/pending decisions, with
instrumentation cost qualified. Existing accepted interval sums can establish
software-event coherence, but cannot recover missing edge arrival times.
Do not add another full-IRQ observer that already failed CPUCHECK, broaden
the mask, or relax3031us based on this table. Prefer auditing the existing
pending/gate/reject path and capture capabilities before adding firmware work.

Run `python scripts/drv_cycle_fault.py <capture>` for clock_closure fields.
Regression tests pin all four differences and require explicit non-latency,
non-independent-clock and non-quantization verdicts.
