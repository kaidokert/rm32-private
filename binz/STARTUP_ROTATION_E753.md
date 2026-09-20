# E753 — restore rotation before interpreting comparator feedback

The operator reported buzzing without movement on E752. Its comparator
accepts and completed seed are therefore not evidence of rotor BEMF.

## Intervention

Return to software-sampled sine startup, with the archived ACK-paced host
50-to-200 eHz trajectory, rather than the replacement timed-ADC startup.
Keep the newer nominal average-current feature, DMA guard publication, bus,
driver, tracking and deadline protections. Retain lean-core (no core event
recorder), but omit lean-irq, startup-adc, and first-accept diagnostics.
This changes several coupled execution paths; it does not isolate the exact
replacement-startup defect.

Release build: opt-level s, thin LTO, one codegen unit.

The first full-recorder candidate was rejected before flashing because its
TIM16 ISR contains __aeabi_uidiv. Adding lean-core removes that direct call.
The audit is not a transitive proof that all interrupt arithmetic is cheap.

Installed frozen image:
`captures/reference/softwarestartup_753b/shell-pwm.elf`

SHA256: `dd860da9fbc4fed6b3623cd4828ad8076b3962d8750a4d41f2648a06c77c0be0`.
Build, download, OpenOCD reset and own disabled guard check succeeded;
guard check reports three timer faults, eighteen post-stop refusals, outputs off.

## Exploratory 30% ramp

Capture: `captures/softwarestartup_753b_start62_bemf70.txt`.
Sine duty 6.2%, driven seed 6.1%, phase shift 60 degrees, initial BEMF 7%.
Host frequency steps 60 through 200 each acknowledged, starting at 3.2 s.
Live duty 10, 15, 20, 25 and 30% all acknowledged.

Operator: “spun pretty sportily now”. Actual rotation restored.
The last duty reply has interval 284 half-microsecond ticks, corresponding
to approximately 1,174 electrical Hz in the controller estimate.

The powered segment stopped at 6,024,087 us, before the requested 30 s:
Bus fault 6, minimum ADC-derived bus 8,381 mV below the 8,400 mV threshold.
21,576 applied commutations, peak absolute current-channel deviation 888
raw counts, nominal average-current cause zero. Last accepted event was
6,024,057 us and feedback acquisition 6,023,801 us: this is not a tracking
loss or stale-feedback stop. Final output-off readback passed; UART closed.

The bus dip is a firmware ADC observation, not an independent PSU display
measurement or proof of its electrical cause. No voltage guard was widened.
The nominal average-current conversion remains uncalibrated, software-startup
sampling is not a uniform time average, and the legacy raw phase-peak limit
is still linked. This is an exploratory comparison, not a production or
sustained-30% qualification. A same-image 25% hold follows separately.

## Same-image 25% ramp and hold

Capture: `captures/softwarestartup_753b_start62_bemf70_ramp25.txt`.
All live steps through 25% acknowledged. Powered window completed normally:
30,000,006 us, deadline reason 2, 177,445 applied commutations, last accepted
event 29,999,998 us, feedback acquisition 29,999,885 us, minimum bus 9,790 mV,
peak raw deviation 823. Final output-off verification passed; UART closed.
The operator reported current capped at 812 mA, approximately the configured
800 mA PSU limit. This observation is specific to this run.

AVGDIAG residual -3670 and cause zero do not corroborate the positive supply
current. The nominal sample-mean guard cannot be represented as validated
average supply-current protection. Investigate offsets, signs and sampling
before relying on it independently of the PSU limit. No current threshold
was retuned from this observation.

Lean-core omits accepted-event statistics, so ACCEPTQUALITY events=0 is not
zero physical accepts. This run has no measured qZC or interval sigma and is
an exploratory sustained-run result, not lock-quality qualification.

## Next work

### E754 voltage corroboration (read-only analysis)

The 25% capture's S85 CRC/cadence/count checks pass: 149,253 aggregated
scans at 201 us, with one explicitly unaggregated publication. Mean bus raw
1141.318, mean VREF raw 1506.727, factory VREF calibration 1662. Using the
existing 11.94 divider scale gives approximately 11.0095 V from the ratio
of those means. This is not the exact mean of individually converted scans.
Relative to the reported 11.7 V idle supply, that is about 0.69 V average
drop; the firmware's 9.790 V minimum is a 1.91 V deepest recorded drop.
It corroborates sag, but does not prove a persistent exactly 1 V sag at the
25% endpoint: the aggregate includes the initial lower-duty ramp.

The signed current raw means relative to 2048 are approximately -8.090,
-8.963, -6.960 for logical A/B/C. All-negative means reinforce that the
current estimator needs investigation; neither a polarity flip nor an
absolute-value conversion is justified by this alone. No firmware, UART or
motor action was taken during this analysis.

Freeze this rotating configuration. Investigate replacement startup by small
single-variable comparisons against it; do not infer wiring or analog limits
from comparator events while the rotor is stationary. Normal startup restart,
current calibration and diagnostic-free qualification remain unfinished.
