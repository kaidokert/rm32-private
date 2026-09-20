# E634 — response to GRAYBEARD_HIGH_SPEED.md (2026-09-15)

Read the complete116-line memo. Keep the useful questions, not its unsupported
certainties. This response does not change any electrical or timing guard.

## Current: retain ripple as a hypothesis, not a proved benign fault

The proposed gain/shunt conversion has not been independently calibrated on
this rig. Raw counts remain the authoritative retained quantity. More
importantly, the quoted non-fault peaks do not predict the stated24% ceiling:
838 counts at22% scaled proportionally reaches1200 at31.5%, not24%. A line
through350 at7% and838 at22% reaches1200 near33.1%. Neither fit is an accepted
current model; both show that the memo's claimed straight-line prediction
does not follow from its own numbers.

E625's23%-ACK fault scan was logicalA3612/B2110/C434 (deviations+1564/+62/-1614),
not the reported pre-fault summary873. E628's22% diagnostic fault was
A3299/B2079/C1679 (+1251/+31/-369), not its pre-fault summary707. Do not pool
those different-build signatures or declare either harmless. A completed22%
run also exists. Ripple, transient current and timing disturbance remain
distinguishable hypotheses; maxima alone do not prove sampling at end-of-ON,
inductance, or an inevitable duty ceiling. V*t/L also omits initial current,
back-EMF, winding resistance and freewheel history in this rotating experiment.

The memo's static-hold I_psu/duty proposal would require established switching
and recirculation paths, steady conditions and offset/accounting of board load;
it is not automatically a one-point absolute gain calibration. No static hold
or current-threshold change was made. nFAULT is PB14 on this rig, not PA5.
The operator's130mA observation belongs to the earlier8.5% test, not an AM32
30% measurement. A10% voltage-drop kill is not the current implemented bus
floor; do not invent that as an operator specification.

## Recovery: the free-run premise is contradicted by the reference

`../minz/core/src/am32_isr.rs` calls COM `disable_interrupt()` at service.
The accepted-input path arms it; arbitrary missing input does not produce a
perpetually free-running COM sequence. This was also verified in E035 and
corrected in the prior memo. FALCON history must not be conflated with the
linked reference event/timer semantics.

With TEMP_ADVANCE16, `advance_of` and `wait_time` give83 HALF-us ticks at
ci333:41.5us, not83us. T/2 without advance would be83.25us, but that is not
the configured wait. The next deadline is measured from the actual edge;
callback latency must still be subtracted. The exact reference arithmetic
is exercised by `scripts/test_recovery_budget.py`. No fake fresh timestamp,
skipped edge or unauthorized free-run scheduling follows from the memo.

E634 stages a pure next-edge protocol: consume the qualified acquisition,
retain its cycle/order history and original deadline, and accept exactly one
further physically qualified edge. The previous twelve-interval mean remains
explicitly provisional; the new onset and measured follow-up interval are
separately reported. Hardware/controller integration is still pending, and
unchanged20us persistence+32us arm floor remains insufficient near1kHz.

## CPU: investigate the DMA work, but use the actual cadence and cost type

`adc_stream::PERIOD_US` with bench-dma-201 is201us (~4.975kHz), confirmed by
DMAFEEDBACK in powered captures. The128–129us cadence was the bridge-OFF
fast-coast diagnostic, not powered DMA. Moving powered scans to250us would
reduce their rate by19.6%, not halve it. It needs an explicit delivery-gap/
queue/deadline review and cannot inherit qualification by arithmetic alone.

The38–52us numbers are observed handler maxima, not a measured40us mean.
Adding maxima from preemptible handlers as mean occupancy can double-count
preemption and does not establish >100% occupancy at a projected1.3kHz.
Measured IRQ union72.9% at~925Hz and75% at~1002Hz does establish that CPU cost
matters. The two soft divides in powered_timer::convert are real and worth
reviewing; any replacement must preserve guard conversion/age semantics.
Moving safety publication back behind UART-dependent foreground work would
reintroduce the defect E624 fixed. No CPU ceiling or30% impossibility is proven.

Current build2DB8 has one7% recovery pass. Historical8%3/3 belongs to32B1,
not2DB8;911A and0D64 have their separately logged single8% passes. Running
qualification at20% belongs to911A. Keep those cohorts separate.
