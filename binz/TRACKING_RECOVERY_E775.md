# E775 — live 30% envelope and ordinary tracking recovery

## Result

The rig now has the practical control sequence requested by the campaign:

1. low-duty 50→200 eHz startup and fresh BEMF handoff;
2. live throttle in 5 percentage-point steps up to 30%;
3. tracking-loss shutdown with the bridge disabled;
4. one-second coast, then the same ordinary startup path;
5. restoration of the last acknowledged throttle through the guarded live PWM
   writer, again in 5 percentage-point steps.

The operator raised the PSU ceiling to 1.5 A. The build carries an explicit
`bench-current-1500` feature; the signed-average guard target is 1.5 A nominal.
The board estimate is known to read roughly 10–15% conservatively high. Bus
sag, nFAULT, tracking, handler/runaway watchdogs and the ratings-derived
same-sample clamp remain unchanged.

Installed/frozen ELF SHA256:
`2BD1C70CCEE48C95C5D58A409279ED5347E67F4FFECA8294C93DB329B4651C62`

It is release `opt-level=s`, thin LTO, one codegen unit. The emitted linker
audit found no forbidden soft division/remainder, wide integer or float helper
reachable from DMA1, ADC/COMP, TIM16 or TIM6. Disabled preflight passed all 3
timer-fault cases and 18 post-stop write refusals.

## Why there are two tracking injections

Comparator suppression is an electrical disturbance: it holds the last
commutation sector until the missing-event watchdog expires. At 20% and 30%,
the average-current guard correctly won that race with reason25. Those runs
were retained and no restart was attempted, because ordinary restart accepts
only tracking reason8.

The separate immediate tracking-decision injection invokes the real reason8
safing path without manufacturing that half-millisecond stalled-sector dwell.
Low-duty E770 already proved that actual missing events reach reason8. The
immediate injection isolates the remaining question: after a tracking decision,
does high-duty ordinary restart and throttle restoration work?

## Exact E775 cohorts

The restoration rate is 5% every 2 seconds. Startup and handoff stay at their
proven 6.1% acquisition / 7.0% BEMF / +60° settings.

| target | run | final reason | powered stop | estimate | bus minimum |
|---:|---:|---:|---:|---:|---:|
| 25% | 01 | 2 | 19,237,008 us | 1,182.0 eHz | 11.737 V |
| 25% | 02 | 2 | 19,237,007 us | 1,153.4 eHz | 11.796 V |
| 25% | 03 | 2 | 19,238,005 us | 1,157.4 eHz | 11.904 V |
| 30% | 01 | 2 | 19,238,005 us | 1,366.1 eHz | 11.677 V |
| 30% | 02 | 2 | 19,237,004 us | 1,344.1 eHz | 11.737 V |
| 30% | 03 | 25 | 14,043,567 us | 1,333.3 eHz | 11.641 V |

Every run made the first 30/25% request, injected tracking reason8 at roughly
5.000 s, held outputs disabled for one second, completed ordinary startup and
a fresh handoff, and applied the expected four or five restoration steps.
Commit-side masked work remained at 8 us and all exits verified outputs off.

Therefore:

- **25% tracking recovery is 3/3 on the exact E775 image.**
- **30% tracking recovery is 2/3 and is not qualified.** The retained third
  run reached 30% but later hit the average-current guard. Do not erase that
  result by retrying until pass or increasing the threshold.
- A preceding E772 exploration ramp held 30% for the full 30-second powered
  deadline at about 1.18 keHz with 11.748 V minimum bus. Thus 30% is a real
  operating point, but its post-restart current margin is not yet reliable.

The lean build intentionally omits per-ISR timing recorders. Zero COMP/COM
recorder fields mean “not linked,” not zero CPU consumption.
