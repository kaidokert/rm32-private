# Projected limits around 50%: the road to full throttle on the G071

Recorded: 2026-09-23, America/Los_Angeles. Author: binz review agent.

Status: historical engineering assessment, not a qualification result, active
goal, or permission to change protections. Written after the firmware50 E152
offline review. No board access, flashing, powered experiment, or firmware
change was performed for this assessment. Append later verdicts rather than
silently rewriting these predictions.

## Assessment in one paragraph

I expect the first barrier to be **event-to-switching latency and interrupt
scheduling**, followed by comparator-service workload and high-duty sensing
assumptions. I do not see evidence that the 64 MHz Cortex-M0+ is fundamentally
incapable of full-throttle six-step control. Full throttle looks plausible on
this MCU with a suitably organized implementation; the achievable speed,
current and thermal envelope of this particular motor and load remains
unproved. The current rewrite's limits must not be promoted into silicon limits.

## The AM32 counterexample must stay in view

The operator explicitly reminded us that AM32 runs on STM32F050 and on this
G071. The F050 platform statement is recorded here as operator-provided context,
not a target audit performed for this note. AM32's operation on our G071 rig is
already part of the bench history.

That is strong counter-evidence to explanations such as "an M0 cannot do this"
or "six-step control inherently needs most of this CPU at modest speed."
Before declaring a fundamental MCU barrier, compare the actual execution and
peripheral architecture with the working reference. Do not assume our ISR size,
priority arrangement, instrumentation, or guard cadence is intrinsic to the task.

Conversely, reference support for a chip is not a matched-condition measurement
of 100% throttle on this motor, load and supply. Neither that result nor full
speed parity is claimed here.

## Evidence available when this was written

- Legacy binz qualified true >=30-second target dwells at 50%, plus restart
  cohorts, on frozen ELF SHA256
  `0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0`.
  The separate rate-curve diagnostic reported about 2096 eHz at 50%; do not
  transfer diagnostic timing costs into the lean image's qualification.
- That successful legacy image raised COM above COMP at live duty >=48% and
  preserved accepted-sector identity across preemption. The preceding paired
  diagnostic found COMP still active at the scheduled deadline in 1869/2031
  sampled events. This supports a scheduling hypothesis, not unique causation
  of the sag failures.
- The clean rewrite, firmware50 through E152, has repeated clean high-duty
  runs but **50% remains unqualified**. Three of six campaign-7 50% attempts
  stopped on fast sag. The mechanism remains unresolved; neither "slip,"
  "supply-limited," nor "mathematically unreachable" is established.
- Its reported 11 us `spent_max_us` is a partial software-timestamp bracket,
  not a full physical-edge-to-timer-arm bound. Nonzero late-arm counters exist
  in four retained production captures, including a 45% run. They are not yet
  localized to startup, ramp, or target hold.
- `com_late_max_us` is service lateness against a software-scheduled timestamp.
  Its maximum includes different TIM16 purposes: commutation and blanking/
  unmask service. It is not, as currently aggregated, proof of 8-11 us delay
  on actual bridge commutations, nor of the hardware timer firing late.

Sources: [legacy campaign](DUTY_50_CAMPAIGN.md),
[legacy reference notes](AGENTS.md),
[rewrite notebook, E142-E152](firmware50/LAB_NOTEBOOK.md), and
[rewrite timing estimates](firmware50/WCET_ESTIMATES.md).
Those documents contain superseded conclusions; use the corrections and raw
captures, not an isolated older summary. Historical installed-image statements
are not a fresh readback of the board.

## 1. Deadline latency is likely to bind before average CPU capacity

Throttle is not CPU utilization. The PWM peripheral generates carrier edges;
100% requested throttle does not imply twice the CPU work of 50%. Work scales
primarily with electrical speed, rejected comparator services, and periodic
housekeeping.

For six-step commutation, `sector_us = 1,000,000 / (6 * electrical_Hz)`.
At a 64 MHz core clock:

| Electrical speed | Sector duration | Core cycles per sector |
|---|---:|---:|
| 2.1 kHz, near the demonstrated 50% point | 79.4 us | 5080 |
| 4.0 kHz, illustrative higher-speed case | 41.7 us | 2667 |
| 4.5 kHz, illustrative higher-speed case | 37.0 us | 2370 |

The last two rows are scenarios, **not predictions of this motor's speed at
100%**. Duty-to-speed extrapolation depends on load, bus, losses and timing.
The core frequency is documented by
[ST](https://www.st.com/en/microcontrollers-microprocessors/stm32g071r8.html).

Thousands of cycles per sector are a substantial budget for six-step control,
but only a fraction is available between an accepted crossing and the desired
commutation. With the present advance formula, at approximately 4 kHz the wait
is roughly 4-7 us for levels 26 through 22. The existing partial 11 us response
already exceeds those windows. Average CPU headroom cannot recover a missed
deadline.

The desired shape is a bounded crossing qualification and deadline-arm path,
an on-time sector update, and bookkeeping outside those critical prefixes.
Absolute-deadline scheduling or hardware edge timestamps can improve timing
fidelity; timestamping alone does not make the qualification code execute faster.
Hardware commutation is an option, not yet a demonstrated necessity.

COM-top is particularly relevant because the successful legacy reference
already used it. It cannot be copied as a naked priority write: the rewrite's
`Seam::root` relies on COMP/COM/DMA being non-preempting peers for exclusive
mutable access. COMP currently arms COM inside an estimator borrow that COM
also accesses. Priority changes require a valid ownership/publication design,
including accepted-sector snapshots, before any powered test.

Prediction: reducing and bounding actual event-to-switching delay will improve
the usable high-speed envelope more than further tuning of qualification-score
thresholds. Falsifier: measured actual switching deadlines remain comfortably
met through the failures, and a matched scheduling A/B does not change them.

## 2. Comparator chatter can turn a feasible budget into saturation

At 4 kHz electrical there are 24,000 legitimate commutations per second.
Several rejected COMP services per accepted crossing can multiply CPU demand.
A small accepted path is insufficient if rejection, retry or blanking service
is expensive.

Measure accepted and rejected service rates, their bounded costs, and blocking
separately. Favor cheap rejection, appropriate sensing windows and bounded
rescue work. Do not "fix" overload by raising the storm cap without accounting
for its service budget and protection deadlines.

Prediction: excess comparator dispatch work, rather than useful commutation
work alone, is a likely next CPU bottleneck after scheduling is repaired.
This is not yet a measured full-speed occupancy result.

## 3. High-duty sensing must remain valid

As duty approaches full scale, the PWM OFF interval shrinks or vanishes. Any
witness, sampling rule or filter that requires an OFF window must have a valid
high-duty alternative. This does not automatically mean the production BEMF
detector is OFF-window dependent; audit the actual path rather than importing
that assumption from an ADC diagnostic.

Current sampling must remain representative, and bus/current protection must
still respond within its specified time even when commutations become faster
than an ADC scan. A scan need not accompany every sector to be useful; its
latency and electrical meaning must be explicit.

Audit fixed boundaries before calling them hardware limits. For example,
firmware50's `SECTOR_FLOOR_US = 40` corresponds to about 4167 eHz. Its comment
calls this physical, but this note does not establish a motor-derived reason
for that value. The fixed 64 COMP-services/ms cap similarly needs a workload
analysis at the intended speed. Neither observation authorizes removing a guard.

## 4. Power, motor and thermal limits remain independent questions

Once control timing is correct, load torque, winding/iron losses, bridge loss,
temperature and available supply headroom may set the usable full-throttle
point. Current at 50% cannot be safely doubled to predict current at 100%.
The existing low-duty current fit is a planning aid, not a high-duty rating.

More PSU current does not repair bad commutation. Equally, every future sag
must not be declared a control defect without evidence. Retain the fast-sag
stop and other electrical protections while determining cause. The earlier
physical incident is a reason to distinguish sustainable operation from a
brief successful acceleration.

There is also an important correction to older advice: the **DRV8304 explicitly
supports 100% PWM duty using its charge pump**, per
[TI](https://www.ti.com/product/DRV8304). A blanket bootstrap-refresh argument
does not impose a 97.7% ceiling on this driver. Full throttle may still map to
less than literal 100% compare for control/sensing reasons, and the timer/bridge
endpoint must be implemented correctly. AM32's chosen numeric duty ceiling is
not by itself proof of a gate-driver limitation on this rig.

## How to judge this assessment later

Preserve exact images, motor/load identity, direction, bus and carrier settings.
For each newly established operating point, record:

1. True target dwell, actual duty, independent or explicitly qualified speed
   evidence, and electrical/thermal behavior.
2. Physical-edge/accepted-edge timestamp definitions, actual timer-arm and
   bridge-update timing, and lateness separated by event purpose and run stage.
3. Accepted/rejected COMP rates, COM/blanking services, guard/DMA costs, and
   non-overlapping occupancy or explicitly inclusive measurements.
4. Measurement effects: which image includes which probe and its cost. A
   diagnostic failure does not automatically delimit the lean image.
5. Which predicted bottleneck actually appeared, what changed it, and which
   predictions were disproved. A successful AM32 comparison must be matched in
   quantities and operating conditions, not merely throttle percentage.

Do not replace a demonstrated measurement problem with a wider pass band and
call it performance. Do not make an uncalibrated measurement the product's
speed ceiling either. Keep qualification metrics distinct from electrical
shutdown mechanisms and account for both.

## Historical verdict to revisit

**My expectation is that this MCU can support a competent full-throttle
six-step ESC. The current implementation's timing architecture is the first
thing I would change, not the MCU.** AM32 is the standing counterexample to
claims of fundamental M0 inadequacy. Reaching a sustainable full-throttle
point on this particular motor remains an experiment, not a promise.

Later validation/correction entries: not yet recorded.
