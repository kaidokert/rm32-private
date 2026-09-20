# G071 architecture budget — E297

## E356: retained decision counts constrain the next optimization

Full campaign verifiers pass for all three archived captures below. These are
instrumented older builds near280eHz, not path measurements of installed28DA.

| Capture | Closed / dispatched | Open, no acceptance / dispatched | IRQ union |
| --- | ---: | ---: | ---: |
| comppaths_hold62_01 | 14.825% | 73.627% | 70.126% |
| singlecore_hold62_01 | 17.455% | 73.132% | 62.471% |
| singlecore_reentry62_30s_01 | 17.475% | 73.082% | 62.795% |

Thus pre-gate deferral alone does not address most recorded handler visits.
These are decision fractions, NOT CPU fractions or a prediction of saved time;
changed scheduling can change later inputs. Open/no-accept is not independently
proven persistence rejection. Current no-counter build lacks this partition.
Report now emits explicit nullable decision fractions, leaving removable CPU
and persistence rejection unknown;300Python tests pass.

Source audit also confirms bench-inline-comp already implies bench-cached-comp:
mode/polarity/trace flags are captured per invocation, while signal reads remain
live. Reintroducing that cache is not a new optimization. Shared core first
tests pending and strict interval>average/2, then clears pending and performs
the live persistence loop. Closed/post-crossing deliberately leaves pending;
a blanket acknowledge would change reference behavior. No shared-core edits.

Next work must target the open-gate workload or its sensing waveform and measure
its effect, not assume all90% nonaccepting visits can be blanked. A concrete
candidate requires preservation of live persistence and acceptance timestamps;
hardware filtering and startup-wide hysteresis failures remain relevant.
Installed28DAC42D and last outputs-off E355 unchanged; no hardware run E356.

Duty expansion is paused for workload characterization, not capped by a new
speed target. User campaign permission remains at most30% duty. AM32 remains
a reserve comparison, not the next required flash or a dependency.

## Measured workload, not a throttle extrapolation

`scripts/drv_architecture_report.py --reentry captures/inline24_reentry62_30s_01.txt`
first runs the complete recovery/CRC/ownership/electrical/deadline/off verifier,
then decodes the measured IRQ union. The retained945D6788 build is
release/s/thinLTO,24kHz BEMF carrier,6.2% duty,trace0.

| Quantity | Recovered segment measurement |
| --- | ---: |
| Electrical speed | 279.002 eHz |
| COM / accepted events per second | 1674.007 / 1674.007 |
| Comparator handler visits per second | 15545.216 |
| ADC scans per second | 4975.111 |
| Comparator visits per COM | 9.286 |
| Comparator visits without accepted events | 388243 / 435097 =89.23% |
| Software IRQ union | 67.370% |
| Mean commutation period | 597.369 us |
| Theoretical64MHz cycles per mean commutation | 38231.6 |
| Maximum COMP / COM / atomic commit brackets | 81 /135 /48 us |
| Actual recovery arm spare above32us floor | 10 us |

The89.23% is NOT removable load: necessary noise rejection, closed-gate
visits, retained pending re-entries, dispatch skips and safety-refused events
have not been separated in this trace-off capture. Nor does the union identify
each vector's exclusive cost. Maxima include preemption and are not additive;
do not multiply them by call counts to manufacture a utilization estimate.
Foreground time is not idle. Do not project this table linearly against duty,
electrical speed or an assumed full-throttle RPM.

Three workload classes need separate models:

- Feedback cadence: TIM3 ADC every201us; keep complete-scan age and raw-current
  protection, distinguish it from optional histogram/statistics work.
- Comparator visits: driven by electrical signal, switching, pending behavior
  and software read aperture; currently about9.3 visits per accepted event.
- Accepted event/commutation work: scales with actual electrical events and
  includes guarded output writes, reference state and retained evidence.

## Next measurement before changing scheduling

Add bounded, fixture-only aggregate counts at the actual reference decisions:
closed interval gate; open gate but no acceptance; accepted; dispatch skip;
guard-stopped/unknown. Capture the FIRST actual `Interval::count()` value for
each COMP invocation, not an extra count read before entering the controller.
Distinguish reference acceptance from recorder rejection after a safety stop.
Do not enable per-read trace bookkeeping merely to obtain whole-run counts.
Retain the existing trace-off build as baseline and measure counter overhead.

This determines whether timed blanking can remove a substantial part of the
traffic. The retained tail alone is neither a whole-run denominator nor a
representative sample: it is deliberately biased toward the final events.

## Deferred sensing experiment contract

The reference test is STRICT `TIM2_CNT > average_interval >> 1`. TIM2 measures
age since the last accepted input (reset in acceptance), NOT time since COM.
COM fires at a separately computed wait and then enables sensing. A fixed
delay after COM is not equivalent. Aroundci1206half-us ticks, idealwait is
about301ticks and gateopens at604ticks; actual COM work/preemption consumes
part of that gap. Re-read current count when scheduling, never assume the gap.

If pre-gate traffic justifies deferral, an optional timer wake should preserve:

1. Same underlying interval counter and strict boundary, with current count
   rechecked at wake; no fabricated accepted event or free-running COM.
2. Reference pending semantics. A pre-gate post-crossing level deliberately
   leaves EXTI pending. Blindly clearing pending at wake loses that behavior.
   Audit G071 EXTI/NVIC masking separately before using it as a latch.
3. Atomic owner/epoch/sector validation at wake. Stop, dropout, panic and
   re-entry revoke the pending wake; an old timer must never re-enable sensing
   for a new or stopped session. Cancellation precedes restoration.
4. Existing independent electrical/feedback-age/tracking/deadline/IRQ-storm
   guards. Deferral cannot refresh accepted-event age or weaken persistence.
5. A resource/priority audit for the wake timer, bounded ISR work and explicit
   counters for scheduled/woke/cancelled/late/refused paths.

Test reference-equivalent edge sequences and cancellation offline, then the
real masked/pending behavior with ENABLElow before any powered comparison.
Measure visits saved, actual COM latency and recovery at the existing qualified
point. Neither an architecture sketch nor an absent desync flag proves gain.

## Status

E342 outcome: peerexti_hold62_01 passes10s at280.447eHz, IRQ58.871%,
COMP18757/s,COM/accepted1683/s,COMmax69us. E340 peerTIM2 hold was63.562%,
COMP16874/s,COMmax75us. Lower occupancy despite MORE visits demonstrates
that visit counts alone are not a CPU budget. Same guards, trace0, carrier,
duties and priority; n1 per build, not recovery qualification or a floor.

E342 comparison contract: keep peer COMP/COM64 and guard/DMA0, release/s/thinLTO,
24,006Hz carrier, startup/BEMF62/62, trace0 and all existing guards. Remove only
bench-filter-bypass from the E341 feature set: normal BEMF delivery returns to
EXTI/ADC_COMP; TIM2 source diagnostics remain compiled but do not own control.
COREPRIORITY now reads ADC_COMP for this build, TIM2 only for filter-control.
First powered comparison is one10s hold, requiring full fixture completion,
bus>=8400mV, raw deviation<=1200, actual seed arm>=32us, arm cost<=16us,
all tracking/ADC/deadline checks and finaloff. Measure rather than prescribe
an occupancy improvement. Retain refusals; do not expand duty on a failed run.
Disabled priority/pulse/atomic/roles/CPU/archive checks precede power.

The existing non-union irq_accounting mode already separates nested vector
time. It is an alternative measurement build, not a free extra counter: clock
reads at every nested boundary and per-context updates perturb the workload.
Qualify its disabled overhead and live behavior separately before interpreting
exclusive times. Current union-only captures cannot supply that breakdown.

E299 result: comppaths_hold62_01 completed10s at279.448eHz,16767COM and
accepted events. Dispatched145198 =21526closed +106905open-no-accept
+16767accepted; no-gate and unknown zero. Closed visits14.8%, open failures
73.6%. Thus pre-gate deferral alone does not address most observed visits.
No exclusive time split is available. Investigate post-gate qualification and
PWM coupling next, preserving the reference persistence requirement.

Instrumentation is NOT timing-neutral: IRQ union70.126% vs prior67.260%,
COMP max85vs81us, COM max268vs134us, cycle sigma51.147vs43.983us.
This is an unmatched-in-time n1 comparison, not an isolated cycle-cost estimate.
Commit max48us unchanged; raw247,bus10925mV,arm51us/cost14,DMA18us/queue3,
stack4072, full fixture and outputs-off passed. No higher duty attempted.

E299 predeclared first live counter comparison: candidate FDD9319B installed;
disabled role6/6, CPU pair maxima2/6/10us, archive3x3 passed. One10s hold at
6.2% driven duty, phase60, trace0,24k carrier, no dropout. Require complete
existing fixture gates: bus>=8400mV, raw deviation<=1200, actual arm>=32us,
arm cost<=16us, valid tracking/ADC/CRC/deadline and finaloff. Additionally require
COMPPATH presence, accepted equality, dispatched<=calls, inactive dump. Any
failure stops expansion; compare timing/IRQ union against inline24_hold62_01.
Counter overhead and sensing perturbation are unknown until measured.

E298 adds optional `bench-comp-paths`, not installed. It captures the first
actual reference interval read per dispatched COMP call and classifies no-gate,
closed-gate, open-without-acceptance, accepted, or stopped/unknown. Recovery
resets the aggregates at the observation boundary. It does not change the
reference scheduling, read count, or guards; instrumentation overhead remains
unmeasured and can change timing. Open-without-acceptance is deliberately NOT
labeled a proven persistence rejection. Dispatch skips are outside this scope.

`drv_comp_paths.py` validates optional metadata; architecture reporting checks
accepted-event equality and the dispatched upper bound. Before live use, add
fixture-required provenance, run disabled preflights and measure at the existing
qualified point. Do not infer removable CPU from these counts alone.

E297 adds a validated reporting tool and this measurement/experiment contract.
No firmware edits, flash, serial action or motor run. Installed945D6788 remains
unchanged; last verifiedoff isE296. The full duty-led goal, calibrated-current
gap, archived parity limits and wider operating envelope remain open.
