# E764–E765 — ratings clamp, arithmetic-clean ISRs, and the 1 A envelope

## Protection changes

The historical absolute `2048 +/-1200` phase check no longer owns powered
protection when the average-current subsystem is enabled. A coherent scan now
uses the same-wake per-channel zero and a **1.3 V** maximum CSA displacement.
That leaves 100 mV inside the DRV8304's specified `VREF/2 +/-1.4 V` linear
output range. At nominal gain10 and7 mOhm this is about18.6 A instantaneous;
it is a sensor-range backstop below the CSD88584Q5DC's50 A headline rating,
not a PCB thermal-SOA certification. ADC rails remain immediate stops.

The lean DMA bus guard now compares the raw bus/VREF ratio with bounded u32
products. Its threshold never accepts a frame rejected by the former
`>=8400mV` conversion; boundary tests cover every VREF code around the decision
for representative/full-range calibration codes. Foreground diagnostics may
still calculate millivolts.

## M0 arithmetic

The sine ISR now receives a foreground-prepared amplitude. Division by255 is
an exact shift/add fold over the complete bounded product domain; the factored
waveform is exhaustively proven equal to or one timer tick below the former
formula. The driven startup successor uses an explicit 6→1 branch rather than
`%6`.

`audit_linker.py` now fails every link if forbidden soft arithmetic is directly
reachable from DMA1, ADC_COMP, TIM16 or TIM6. Frozen E765 ELF SHA256:
`964e160b9c038358e54d6f5a105fc89ad84dc23c5bfeb278df6327c0d89a42dc`.
All four roots have zero reachable division,64-bit,128-bit or floating helpers.
The audit remains explicit that indirect calls and inlined wide sequences need
manual inspection.

## Hardware evidence

E764 first proved the new ISR paths and clamp at15% for10 s. It then ACKed the
full live ladder10→15→20→25→30%. At30%, tracking remained good and the signed
average-current guard stopped at5,362,190 us, reason25; bus minimum11.569 V.
A25% run likewise stopped on current at7,262,030 us, bus minimum11.653 V.

One20% attempt exposed the final shadow copy of the old `+/-1200` foreground
startup cutoff. It stopped at3.206 s before BEMF. That exact check was replaced
by the ratings-derived same-wake clamp; this was a causal code fix, not a retry.

E765 then produced the envelope bracket:

| commanded duty | result | powered stop | bus minimum | tracking |
|---:|---|---:|---:|---|
|20%|full30 s deadline, reason2|30,000,009 us|11.617 V|fresh/no event fault|
|22.5%|average-current stop, reason25|20,084,624 us|11.820 V|fresh/no event fault|

The current1 A sustainable envelope is therefore bracketed between20% and
22.5%. Commanded30% and live throttle operation are proven, but sustained30%
is not available under this current limit. None of the higher-duty stops were
bus, nFAULT, tracking, ISR watchdog or BEMF failures. All fixtures verified
final outputs off and closed UART. E765 remains installed and disabled.

This is envelope exploration, not a new formal qualification cohort. The
first20% run was subsequently joined by two predeclared unchanged-image runs.
The strict lean verifier (parameterized for duty without changing electrical
criteria) passed the three captures:

| run | stop us | commutations | event age us | feedback age us |
|---:|---:|---:|---:|---:|
|1|30,000,009|155,645|64|124|
|2|30,000,004|157,269|31|120|
|3|30,000,005|158,254|111|120|

All ended on powered deadline reason2, had no event fault/veto, and verified
final outputs off. Foreground `COASTREF` may report1 when it observes the same
deadline first or7 when it observes the powered owner already stopped; the
verifier permits only those two same-deadline orderings while requiring the
authoritative `POWERPATH reason=2`. Thus20% is now a formal representative
lean qualification as well as the lower side of the1 A envelope bracket.
The earlier E76215% cohort remains independently valid.
