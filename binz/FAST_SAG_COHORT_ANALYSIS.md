# Reverse 45% fast-sag cohort: bounded timing read (2026-09-19)

## Carrier-only 48 kHz follow-up (one pass, one stop)

The48k image changed only the reverse PWM carrier from the32k
`SAGPWM` build. One protected run held45% for~18s and ended by
intentional HostAbort9 with no electrical fault/foldback. A second
stopped on the same reason26 fast-bus rule after ACK46.5%; 50 was not
reached. The terminal PWM snapshot was stamp43500632us, CNT966,
ARR1332, CCR619, upcount. The final three acquired stamps were
43500108/334/560us. Applying the same bus aperture +42.46875us
gives bus PWM counts807/608/409, respectively OFF, near compare,
mid ON; normalized bus93.24/94.21/92.58%. The three-scan bus
depression therefore is not a single PWM-edge notch at48k either.
There were no current-foldback, tracking or nFAULT stops, and the
accepted-event tail does not show a held sector. The duty/ramp exposure
and n=1 at each carrier forbid a claim of a reliable+2% advantage.
This A/B does show that simply raising the carrier did not eliminate
the bus-collapse failure class. Exact lean32k image restored/off.
Captures:
`captures/reverse_direction_2026-09-19_48k_hold45.txt`,
`captures/reverse_direction_2026-09-19_48k_sag_pwm465.txt`.

## New terminal PWM-phase capture (same day, last ACK 44.5%)

Frozen diagnostic SHA47ECC83A... adds only a TIM1 sampled-clock/CNT
snapshot at the third sub-95% bus scan; the eight DMA scan rows and
all protections are unchanged. Correctly armed run held40% for18s,
then stopped reason26 after ACK44.5, before45 was accepted. The
terminal row is `stamp_us=48361796`, `TIM1 CNT=1284`, `ARR=1999`,
all `CCR=890`, `CR1=129` (enabled, upcount). ADC is PCLK/4=16MHz,
sample160.5 cycles, conversion12.5 cycles; the physical bus channel
is fourth, so its nominal aperture ends42.46875us after each trigger.
With64MHz TIM1 and no carrier rephase in the retained1.58ms window:

`bus_pwm_cnt = (1284 - (48361796-acquired_us)*64 + 42.46875*64) mod 2000`

| Scan | Acquired us | Bus/VREF normalized to1209/1506 | Bus PWM CNT | Relative to CCR890 |
|---:|---:|---:|---:|---|
| 0 | 48360128 | 96.96% | 1250 | OFF |
| 1 | 48360354 | 96.80% | 1714 | OFF |
| 2 | 48360580 | 96.61% | 178 | ON |
| 3 | 48360806 | 97.22% | 642 | ON |
| 4 | 48361032 | 98.25% | 1106 | OFF |
| 5 | 48361258 | 92.58% | 1570 | OFF |
| 6 | 48361484 | 93.03% | 34 | early ON |
| 7 | 48361710 | 92.97% | 498 | mid ON |

The last three are 226us apart and span widely different PWM phases.
The projected counts have clock/ADC launch uncertainty, but that
uncertainty cannot put all three at one switching edge. The VREF codes
remain1505..1507. This is a real short-duration bus depression, not
the single edge-contaminated frame caught by the separate first-peak
probe. Stop at48361812 is~512us after the first estimated low-bus
aperture. The event tail immediately before/on the dip has accepted
gaps~81..106us and COMs continue; no missed-edge/held-sector pattern
appears in this~1ms view. It does **not** prove what demanded current
or whether the external PSU entered current limit. One older136us
accepted gap preceded a recovered98.25% bus scan and is not assigned
as the onset. This image cannot qualify the lean controller at44.5%.
Raw capture: `captures/reverse_direction_2026-09-19_32k_sag_pwm445.txt`.

This is an offline read of the three frozen V3 scan-ring captures, not a
new powered qualification. The exact fixed20 reverse32k lean image remains
installed/OFF with COM41 closed (see `AGENTS.md`). All three V3 ramps
reached 45% and stopped on the unchanged three-scan fast-bus guard;
40%/60s on the same diagnostic image completed normally. No 50% run.

## First: decode the ADC path correctly

The hardware converts ADC channels 0,1,4,6,13 in ascending order.
`adc_stream::poll()` passes that array through
`dma_snapshot::logical()`, which returns `[ADC4,ADC1,ADC0,bus,VREF]`.
The `SAGROW`/`SAGSCAN` labels therefore already are physical IA/IB/IC.
The previous claim that archived IA and IC were swapped was **false**;
the mistaken source-only swap was never flashed and has been reverted.

`shell-pwm.rs` sets ADC PCLK/4=16 MHz and SMPR=160.5 cycles. The 12-bit
conversion adds about 12.5 cycles. Thus one channel takes nominally
173/16=10.8125 µs and the sample/hold endpoints after a trigger are
approximately IC +10, IB +21, IA +32, bus +43, VREF +53 µs. These are
**derived** times, not measured apertures: trigger launch/selection
latency, conversion overlap and the `acquired_us` clock mapping add
uncertainty. `acquired_us` is the programmed trigger origin, while the
controller fields in `SAGROW` are read at later DMA service (~70-95 µs
after origin). A commutation can occur between phase samples or before
the service snapshot; the three phase values need not belong to one
sector or PWM slice.

## What the onset windows actually show

The table uses `acquired_us+43 µs` as an *estimated* bus aperture. The
accepted-event intervals listed are all the intervals that can overlap
the 226 µs between the last near-normal sample and the first terminal
sub-95% sample, not a selective pick of the longest gap.

| Run | Last near-normal bus | First terminal low bus | Accepted intervals across that window | Verdict |
|---|---|---|---|---|
| A | 98.9% at ~40,642,646 µs | 93.3% at ~40,642,872 µs | 90, 77, 93 µs | No late accepted gap in the sampled onset window |
| B | 98.6% at ~38,310,189 µs | 92.9% at ~38,310,415 µs | 92, 101, 72 µs | No late accepted gap in the sampled onset window |
| C | already 93.6% at ~39,431,955 µs | final streak starts 92.2% at ~39,433,085 µs | event ring begins ~39,432,377 µs | True onset precedes retained event history |

In A, the first >140 µs gap ends only after the first low bus sample;
in B there is no comparable gap. C's 138 µs gap also follows its final
streak start, but earlier bus dips are outside the event ring. These
facts argue against a **shared missing-edge/held-sector precursor in
the captured onset windows**. They do not rule out phase-angle error,
current ripple, an earlier control excursion, or a supply/path transient.
The physical bus drop may start anywhere between 226 µs-spaced samples,
and the bus-aperture estimate is not a scope timestamp.

All three 45% ACKs had controller interval averages 176/179/179
half-µs (about 1894/1862/1862 eHz); their later stop values were
195/190/203 (1709/1754/1642 eHz). Speed estimate fell under the same
45% request, but two endpoints cannot determine whether voltage sag
led slowing or slowing raised electrical load. The fixed bus reference
and fast-sag stop prevented holding through the collapse.

## Current and sector: what is and is not established

The phase shunt scale from the configured 10x gain, 7 mΩ shunt and
roughly 3.3 V ADC supply is about **87 counts/A**, or **11.5 mA/count**;
the review's "11.5 counts/A" inverted the units. The ~±2,000-count
samples are therefore near the ADC/CSA range, nominally tens of amps
instantaneously. Zero/gain are not precision-calibrated and each ADC
sample is asynchronous to the PWM carrier. These codes cannot be read
as DC-link average current, a FET SOA certificate, or simultaneous
three-phase current. A/B have near-rail phase samples before or during
the final bus streak; C has several, including IB=0 on the terminal
scan. `phase_rail_codes=0` on C is not contrary evidence because
`scan_raw()` returns on the same-wake fast-bus trip before reaching the
rail counter.

The reverse six-step roles from `phase_direction::physical_step()` and
`phase_gpio_plan::plan()` are:

| Core step | Physical source | Physical sink | Floating |
|---:|---|---|---|
| 1 | B | A | C |
| 2 | C | A | B |
| 3 | C | B | A |
| 4 | A | B | C |
| 5 | A | C | B |
| 6 | B | C | A |

That map is source-grounded, but `core_step` in each `SAGROW` is the
state at DMA **service**, not at IC/IB/IA sampling time. At ~95 µs per
commutation, a ~50 µs age difference can cross COM. Inductive freewheel
can also produce current in a newly floating phase. A static comparison
of one row's largest current to its service-time core step cannot prove
cross-wiring or late commutation. A future phase-attribution capture
needs a sector/COM timestamp aligned to the ADC channel aperture, or
must remain explicitly inconclusive.

## Protection and next discriminator

The current image explicitly reports `pulse_stop=0 phase_rail_stop=0`;
the historical single-sample raw clamp was retired after PWM-ripple
false stops. The 4 A signed-average actuator spans roughly 11 ms, so
it does not react to the ~0.7 ms final bus streak; the independent
fast-bus guard did. Do not claim an inactive clamp silently failed,
and do not restore a raw threshold without calibrated peak/SOA policy.
Changing carrier alone is not the next causal move: reverse40k already
fast-sagged at 45%, and 32→48k would shorten fixed-duty ON-time by one
third, not half.

Before another 45% run, the useful discriminator is a **small,
bounded-cost comparison of 40% versus the onset of 45%**: count severe
phase-current samples and bus-dip *streaks* per unit time, and retain
the recent duty/interval trend with physical phase labels. It should
answer whether the near-rail excursions are ordinary at clean 40% or
rise before the 45% bus collapse. Keep the fast-sag, current actuator,
nFAULT, tracking and watchdog stops unchanged; use a separate diagnostic
image and then return to the exact lean image. A reference AM32 run at
matched speed remains useful only if it can retain equivalent immediate
electrical protection on this replacement motor; old-motor response is
not qualification.

Full captures: `captures/reverse_direction_2026-09-19_32k_scanring45a_fastbus.txt`,
`...45b_fastbus.txt`, `...45c_fastbus.txt`. Frozen diagnostic ELF and
its label caveat: `captures/reference/reverse_32k_sag_scanring_20260919/`.

## Phase-current census follow-up (same day)

The report-only phase-current census image passed 10%/15s and 40%/60s
with all electrical stops intact. At ACKed 40% it saw 223,220 complete
DMA scans over 50.45s and **zero** phase displacements from the same-wake
zero >=1,500 raw counts. Individual bus<95% samples occurred 1,418
times, but no streak exceeded two scans; the fast-sag stop remained
quiet. This is a useful clean baseline, not proof that true current
peaks are absent between asynchronous ADC samples.

The gradual comparison stopped before 45%: the last ACKed duty was
43.5%, and the next command arrived after `POWERPATH reason=11` at
powered 35.259715s. Its 28.00s census since the 40% ACK covered
123,903 scans and counted five phase displacements >=1,500, four
>=1,900 (IA four, IB two, IC zero; per-phase counts can overlap),
including one exact ADC rail. Four severe samples coincided with
bus<97%, two with bus<95%. Here bus<95% appeared only 56 times,
all isolated, and the fast-sag guard did not fire. Thus near-rail
samples emerged in the climb above the clean 40% point, but this
aggregate cannot assign their exact duty or prove which signal led.

`ADCFAULT stage=30` decodes to `DMAFAULT code=10`, the `adc_stream`
path where `Latest::publish` refused a frame. Its policy rejects exact
ADC zeros/full-scale as `Invalid`, as well as stale/reordered frames.
The same run's `exact_rail=1` makes the invalid-frame explanation
plausible, but the exact `Error` variant was not retained, so it is
not proven uniquely. The stop is not a bus-sag trip and is not a clean
45% test. It safed outputs; exact lean firmware was restored with
ENABLE/MOE/CCRs off and nFAULT high. Before another high-duty climb,
resolve the rail-frame subtype and how the asynchronous current
apertures relate to PWM/commutation. Do not disable this stop to force
an envelope number. Raw terminal capture:
`captures/reverse_direction_2026-09-19_32k_phasecensus.txt`.

## First-peak protective A/B: 32 versus 48 kHz (same day)

To avoid asking the replacement motor to absorb more near-rail samples
while locating their onset, an opt-in diagnostic stopped on the first
complete DMA phase sample displaced >=1900 raw counts from its
per-phase same-wake zero. This is a **protective probe**, not a calibrated
current limit or an envelope qualification. The existing active4A
average actuator, fast5%/three-scan bus guard, nFAULT, tracking and
watchdog remained unchanged. 32k and48k each passed a protected
40%/60s control with zero >=1500 phase samples.

| Carrier | Last ACKed duty at first-peak stop | Estimated speed | First physical frame IA/IB/IC | Normalized bus at frame |
|---|---:|---:|---|---:|
| 32k | 43.0%, before43.5 command | ~1792 eHz | 2047 / 1094 / 4087 | ~95.8% |
| 48.012k | 43.0%, before43.5 command | ~1801 eHz | 1977 / 4019 / 1270 | ~98.2% |

Both stopped `POWERPATH27`; neither reached an exact ADC rail or a
three-scan fast-bus trip, current foldback, tracking fault or nFAULT.
At48k the 37.55s census since40% ACK counted seven >=1500 frames and
one >=1900 frame in166,129 scans; at32k the 36.07s census counted one
of each in159,583 scans. Those are **one run per carrier**, and the
first-peak stop censors later events. Neither the equal stopping duty
nor the seven-versus-one moderate-event count establishes a rate or
causal carrier verdict. A 32k->48k change shortens nominal fixed-duty
ON-time one-third, yet it did not obviously move the first-peak onset.

The raw frame is sequential and asynchronous to PWM. The service-time
sector in `TRACKSTOP` cannot assign all three samples to one conducting
pair. The next instrument should cheaply latch the PWM counter and
last COM/event timing at the **first** near-rail frame, then test whether
the excursion consistently samples the end of ON-time or follows a
commutation timing irregularity. Do not equate one clipped shunt sample
with 23 A continuous motor current, and do not weaken the bus/average
stops to get a 45/50% pass.

The 48k test initially hit two tooling/firmware integration refusals,
both resolved without changing electrical protection: valid1333-tick
geometry was absent from `live_duty::Prepared::new` (host tests and
disabled hardware writes now pass), and a six-byte 115200-baud burst
could overrun UART RX (`HOSTABORT` active error0x300). Pacing the six
bytes across~148ms, below the parser's250ms deadline, gave reliable
ACKs through40% and into the upper climb. Those HostAborts are not
carrier/motor failures. Full captures: `captures/reverse_direction_2026-09-19_32k_peakstop43.txt`
and `captures/reverse_direction_2026-09-19_48k_peakstop43.txt`.
