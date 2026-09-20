# Low-hysteresis comparator A/B

E355: qualified28DAC42D restored and passes matched10s hold, outputs off.
Archive: captures/reference/range320_28da/shell-pwm.elf. Candidate779C8978
is no longer installed; its first acquisition failed (E354).

## Evidence and rationale

shell-pwm boot writes COMP2 CSR with HYST=0. G071 PAC places HYST at bits16:17;
the cached upstream HAL encodes None/Low/Medium/High as0/1/2/3. The ST
[G071 datasheet](https://www.st.com/resource/en/datasheet/stm32g071kb.pdf)
specifies typical hysteresis0/10/20/30mV. These are typical values, not measured
bench thresholds. ES0418rev5's summary lists no COMP-specific limitation;
absence from that list is not proof of ideal behavior.

The reference qualification still takes12 live comparator reads here. Advance16
is compiled into the shared COM ISR; altering only the local seed's advance
would mismatch ongoing scheduling. Neither is changed by this experiment.

Hypothesis: analog hysteresis can suppress small near-threshold chatter without
extra runtime ISR work. It cannot reject arbitrary large PWM feedthrough, and
it shifts rising/falling thresholds: it may delay or lose valid crossings,
alter speed/torque and reduce weak-BEMF acquisition. Not a free noise cure.

## Candidate and scope

Featurebench-comp-hyst-low sets HYST=1 in the boot CSR write before the existing
5us settling delay. Applies to startup AND BEMF; no live switch, no register
write on each ISR, no cached expected level, no change to raw persistence,
referencecore,320profile or electrical/age/arm/deadline guards. Existing mux
updates preserve HYST. Fixture COMPHYST records actual final HYST bits and
--low-hysteresis requires code1; wrong/missing/duplicate metadata refuses.
299Python tests and release/s/thinLTO build pass. Candidate
779C8978BD2FA478D78B91FAFA73F7689E4080C9B11E7014FBF640F827A6CFC4 is not installed.

## E354 first attempt: early latched seed refusal

Disabled CSR readback40010281 confirms HYST1. Initial priority fixture lost
UART; exact disabled register checks preceded USART clock restoration.
priority02 passes three trials; preflight01 passes pulse/atomic/roles/CPU/archive.
hystlow_hold64_01 requested10s, startup6.2%, BEMF6.4%, phase60, trace0.
No BEMF handoff occurred: the under-drive window completed20011us, reason2,
with22 accepted IRQ inputs but seed fault3. After one restart at epoch4,
epoch4->5 endpoints4145->5264us give1119us (2238half-us ticks), exceeding
the existing1000us seed interval maximum. This latches at epoch5; the later
window deadline is not the original cause. Later regular inputs cannot revive
the failed qualifier. Raw peak821/1200 counts, bus minimum11366mV, finaloff
verified. No calibrated-current, sustained-jitter or occupancy result here.

One failed entry does not establish hysteresis as its cause. Do not widen the
seed limit or restart indefinitely to rescue this experiment. A controlled
return to the archived no-hysteresis build is the next comparison; any attempt
to isolate hysteresis to BEMF must first validate its disabled-owner transition.

## E355 baseline restoration

Restored exact28DAC42D archive; disabled CSR40000281 confirms HYST0. Initial
UART clock issue retained and repaired after safe register checks, followed by
three priority trials and all five preflights passing. hystzero_hold64_01
uses the same10s/drive62/BEMF64/phase60/trace0 settings. Full fixture PASS:
292.576eHz,17555COM/17554accepted,IRQ58.774%,cycle sigma38.048us,
COMP71/COM69/commit45us maxima,raw267,bus10901mV,entryarm65.5us/cost12,
stack3300,finaloff. Acquisition had14accepts,seed1540ticks andraw500.
Capture SHA1D69BD998026E8711B904D4834517CE5AA65EEC7EEA76DF4D6B240F2BC9CDE6C.
This confirms reference operation survives restoration, not a statistically
established hysteresis cause or a sustained hysteresis comparison. Park this
startup-wide candidate; retain baseline for architecture/scheduling work.

## Qualification order

Verify disabled CSR readback after flash, and existing priority/pulse/atomic/
role/CPU/archive preflights, before power. First10s run at6.4% BEMF/6.2% startup,
phase60,trace0,24006Hz. Require full fixture guards and finaloff. Compare both
speed and cycle variation/IRQ visits with the zero-hysteresis baseline; a
lower speed alone is not improved same-speed timing. Then recovery at that
point before approaching6.7 again. Any refusal remains a failed attempt;
no persistence-count or guard-threshold changes to rescue the candidate.
