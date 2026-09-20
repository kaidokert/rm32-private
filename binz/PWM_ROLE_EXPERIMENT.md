# Free-running PWM role experiment — E289 audit

Status E290: optional BEMF-only live implementation compiled; NOT flashed or
hardware-qualified. Installed image remains idle diagnostic4DA1EE7E; known
motor baseline807372B6. Root ELF is now candidateA54DCFEA (identity below).

## E290 implementation and next acceptance gate

`bench-pwm-roles` changes ONLY `powered_timer::commit`, inside its existing
guard/ownership/write critical section. Forced/open-loop startup remains the
old path at10kHz. This scopes the first comparison to BEMF commutation, not
full reference waveform parity. `phase_role_sequence.rs` orders blank,
initial equal-CCR preparation if needed, GPIO modes, sink, COMP mux, MOE.
All36 sector transitions are host-tested with no ongoing preparation call.
Prepared segments hold a single duty; an unexpected change refuses through
the local safing panic handler instead of partly updating live CCR preloads.
Global bridge_clear clears MOE/CCRs/BOTH GPIO latches, revokes preparation,
then restores AF on all six inputs. That also preserves legacy sine/startup
and diagnostic assumptions on the next invocation. Existing baseline builds
without the feature retain the previous commutation code.

Explicit idle `rolecheck` keeps ENABLE low while exercising MCU gate roles;
it is NOT a motor run. It checks all six sectors' MODER, ODR, GPIO-role IDR,
equal CCR345, PWM modes/CCER, mux, and counter continuity across a repeated
role call. Counter check uses TIM17 elapsed and TIM1 delta modulo6400 with
192tick tolerance for read/quantization boundaries and elapsed<70us. This is
a counter-reset discriminator, NOT a CPU utilization or arm-margin test.
It safes before printing. Required disabled preflight: six flags127 rows,
passed6/expected6, restored1, disabled1, plus independent final off/p/i.
No preflight has run yet. Add strict host parsing/provenance before live use.
The first powered comparison remains gated on this check and fresh disabled
archive checks; keep all electrical/tracking/actual-arm limits and explicit
coretrace0. Do not infer safety/headroom from host tests or flash fit.

Release/s/thinLTO/codegen-units1 candidate SHA256:
`A54DCFEAA449C83AF4300DF3600F7A858FC138026BDF9554633DA01FEAB3B365`.
text118720/data1088/bss29264. Features: bench-driven-reanchor,bench-cpu-union,
bench-range310,bench-reentry-staging,bench-current-baseline,bench-driven-dma,
bench-dma-201,bench-adc-phase,bench-dma-fast-start,bench-irq-tail,
bench-cached-comp,bench-pwm-roles (NOT bench-baseline-dma).
173 Rust tests, M0 no_std check,244 Python tests pass. Existing warnings remain.
No flash, serial operation or motor run in E290.

## E291 live gate (declared before the powered attempt)

A54DCFEA flashed successfully. roles_disabled_01 retained UART-silent refusal;
exact probe registers RCC08000000/PD1ODR0/BDTR0c1a/CCRs0 verified before
documented RCC08040000 UART-clock restoration. roles_disabled_02 passed6/6,
allflags127,8us/492..494carrier ticks each; final restore/off passed.
roles_cpu_01 passed2/7/10us maxima; roles_archive_01 passed3x3/finaloff.
Host role parser validates complete unique rows, counter arithmetic, summary,
post-test off; PWMROLES provenance validated centrally and required by new
fixture --pwm-roles.247Python tests pass.

Next one powered attempt roles_hold54_01:10s BEMF hold,5.4% powered duty,
phase60,coretrace0,unchanged6.2% startup. Require full existing capture/CRC/
ownership/feedback/age/current/bus/cycle/actual-arm/deadline/finaloff checks
and exact PWMROLES metadata. Existing guards include current2048+/-1200raw,
bus8400mV,cycle3226..6000us,event268..1000us,actualarm>=32us,cost<=16us,
commit<=100us. No recovery/duty increase until initial result is inspected.
This compares BEMF waveform implementations at unchanged carrier/duty, not
equal-speed/current parity; record any startup/guard refusal as a failure.

roles_hold54_01 PASS10s268.346942eHz,16101COM/16100accepted,cycle sigma40.544506us,
raw458/bus11092mV,arm57us/cost14us,COMP86/COM100/commit49us,queue2/DMA18us,
stack4128; finaloff verified. Slower than baseline~290eHz atsame duty, not
proof of noise reduction at matched speed. Next roles_hold55_01 one10s5.5%
hold, same phase/trace/startup/all numeric guards and full verifier as above.

roles_hold55_01 PASS10s273.691291eHz,16422COM/16421accepted,sigma39.741862us,
raw446/bus11068mV,arm59.5us/cost14us,stack4128/finaloff. Next one10s hold
roles_hold58_01 at5.8%,sameother settings/guards. Measured5.4->5.5 slope
predicts roughly290eHz, allowing a closer-speed comparison with baseline5.4;
this prediction is NOT permission to pass through a guard or a new ceiling.

roles_hold58_01 PASS10s287.794436eHz,17268COM/17267accepted,sigma39.681237us,
raw456/bus11044mV,arm58.5us/cost14us,COMP86/COM101/commit44us,guard25us,
DMA18us/queue2,stack4128/finaloff. All three holds are single attempts, not
a repeated cohort; recovery on this build remains untested. Near-speed sigma
39.68us vs old cachedoff_hold54_01 41.35us is not a decisive improvement.
COM maximum is now100..101us versus old81us (not an isolated commit measure).
Next inspect this timing tradeoff and test nearer the old~297eHz edge before
claiming the UG change cures it. Do not confuse a higher duty at lower RPM
with an expanded speed envelope. All raw current figures remain uncalibrated.

## E292 next powered gate

One roles_hold60_01 at6.0% powered duty,10s,phase60/coretrace0/unchanged6.2%
startup onA54DCFEA. The5.5->5.8 measured slope predicts~297eHz, the old
intermittent-failure region. Require exactroleprovenance and full existing
fixture checks/current/bus/age/cycle/actual-arm/commit/deadline/finaloff gates
fromE291, with no threshold changes. Inspect result before any next attempt.

roles_hold60_01 PASS10s296.270732eHz,sigma39.531078us,17777COM/17776accepted,
raw453/bus10984mV,arm59.5us/cost13,stack4128/finaloff. One pass at the old
edge, not evidence of reliable entry/recovery there. Next roles_reentry60_30s_01:
same6.0/phase60/coretrace0 with30s ORIGINAL session budget, COMP dropout at2s,
ONE fresh guarded recovery. Require actual injected stop, valid FIRSTSEG,
fresh acquisition/seed, actualarm>=32us/cost<=16us, resumed sustained events,
original deadline and all existing electrical/age/cycle/off checks. Preserve
any refusal, no retries within the attempt or guard relaxation.

roles_reentry60_30s_01 FAIL after actual injected stop/fresh recovery:
FIRSTSEG3572COM/3571accepted,stop2000909us; recovery12intervals7209us,
seed1129. Resumed only92909us/166COM/165accepted thenCycleTiming12,
step3guard89640->92861=3221us<3226. Finaloffpassed. Sameclass persists
without per-COMUG; no cureclaim. Next one30s original-budget injected
recovery roles_reentry58_30s_01 at preceding5.8%, all other settings/guards
unchanged, with same recovery acceptance requirements above.

roles_reentry58_30s_01 PASS: fresh12intervalseed1167 after7516us acquisition,
actualarm39us/cost14us. Resumed27.990049s289.907922eHz48687COM/48687accepted,
sigma36.655782us,raw453/bus11056mV,COMP86/COM101/commit43/guard24us,
DMA18/queue2,stack4000. Originaldeadline163usspare; full verifier/finaloff.
One successful recovery at preceding duty, NOT a reliability cohort or cure.
At6.0% the failed recovery arm was34us/cost14; it armed legitimately then
tripped cycle timing. Do not blame that stop on a refused seed.

Next distinct experiment: carrier frequency, after checking DRV input pulse
requirements and sampling/metadata dependencies. Source audit already finds
fixed6400 inphase_role_sequence,adc_phase_dma/adc_occupancy, and192/320tick
targets withARR6399 assumptions inpwm_sample_dma. Merely changing ARR
would invalidate waveform duty and capture interpretation. Consider limiting
the frequency experiment to BEMF ownership, with explicit startup restoration,
to keep the separately-qualified driven-acquisition timing unchanged.

## E293 carrier geometry and cached driver timing audit (no hardware action)

Read localdrv8304.pdf SHA256
DF052F7308B0C8E0DD8759292AC070DE936788F43F3A54AA16D7C78BABB2DB61,
SLVSE39B. Pages7/8 listHW tDEAD120ns,tDRIVE4000ns and propagation180typ/
250maxns. Page23 explicitly says tDRIVE does not extend PWM and ends on a
new command. Thus4us is NOT a minimum PWM pulse requirement. Page24 describes
input deglitch/digital/analog delays and gate-threshold handshaking. The reviewed
specification does NOT give a guaranteed minimum INHx/INLx pulse; propagation
delay alone cannot supply one. Gate charge/IDRIVE effects still need experiment.

Added pure carrier_profile.rs: explicit6400/2666ticks (ARR6399/2665), exact
frequency floor10000/24006Hz at64MHz; bounded compare calculation, 32bin phase
mapping, idle counter-discriminator arithmetic, ideal MCU high ticks after
unchanged26tick deadtime. At24k candidate4.0/6.0/6.2% gives80/133/139high
ticks =1.25/2.078125/2.171875us, NOT measured gate/conduction durations.
phase_role_sequence now has apply_carrier; existing live caller still uses
the10k wrapper. Five newtests cover bothcarriers, allphasebins, invalidduties,
timingbounds andcandidate initialcompare/unchanged roleordering.178Rusttests
andM0no_stdcheckpass. No24k live option, timerperiod change,flash ormotor yet.

Release/s/thinLTO buildpass,text118712/data1088/bss29264;
rootELF593DA14E34B7F2F07959B4E74DFBAB8F72D385187D4B8BCF5BD832D25E4BBC4A
isNOTinstalled. Hardware remainsA54DCFEA,lastverifiedoffE292.

Integration order: establish chosen carrier while bridge disabled BEFORE
ADC stream start/firstCOM; snapshot that period for retained ADC phase bins,
never reinterpret a mixed-period stream. Existingcore_bench setup clears
bridge before baseline/guard setup; use/audit this boundary rather than
switching ARR during the first COM after ADC may already have started.
Safing restores10k only after outputauthority revoked; any retained histogram
must keep its acquisition period despite this restoration. Keep driven
PWM192/320tick diagnostic andstartup at10k. Update idle rolecheck period/
expectedCCR/continuity/restoration and strict host metadata before flashing.
Measure added seed-setup/IRQ cost; unchanged arm/age guards may legitimately
refuse. No RC network change is implied by the memo or this experiment.

## E294 24k candidate integration and first powered gate

bench-pwm-24k (impliesroles+DMA) establishesARR2665 in disabledcore_bench
setup beforeguard/ADCstart. prepare_carrier requiresoutputsdisabled/noactive
poweredguard/noDMAch1/zeroCCRs. apply refuses an unexpectedARR. Globalbridge
safing restoresARR6399/AF afterMOE/CCRs/GPIOclear. All startup/drivenDMA
192/320 timing remains10kHz. ADCphase snapshots6400or2666 beforestream,
usesmatchingbins and retainsperiodafterstop. Hostmetadata recognizes24006Hz
and2666ticks, requiresmatchingPWMROLES for24kADC; CLI--carrier-hz bindsrun.
Idle--period-ticks bindsrolecheck, nowalsoverifiesARRrestoration.

Candidate3DEAC2F08CCB386876C221756F4D3E5CE2EB9FC75DE6BD893E56F6C0D31C336F
release/s/thinLTOtext119368/data1088/bss29264 flashed.249Pythonpass.
carrier24_disabled_01 UARTsilent retained/exactsaferegs thenknownclockrestore.
carrier24_disabled_02 all6flags127,12..13us/779..781ticks,restored6400/off.
carrier24_cpu_01 max2/7/10;carrier24_archive_01 3x3pass/finaloff.

Nextone carrier24_hold58_01:10s,5.8%powered,phase60/coretrace0,
unchanged6.2%startup,exact24006Hzmarker/ADC2666period required. All previous
current2048+/-1200,bus8400,age1ms,tickgap200us,cycle3226..6000/event268..1000,
arm>=32us/cost<=16us,commit<=100us,finitebudget andfullCRC/ownership/ADC/
timeline/finaloff gates unchanged. Compare preceding10k5.8pass, noassertion
equalnominaldutymeansequalconduction/speed. No recovery until this is inspected.

carrier24_hold58_01 PASS10s254.266759eHz,sigma58.848151us,15256COM/15256acc,
raw245/bus10925mV,ADC49751period2666,DMA18queue3,arm54us/cost13,
COMP87/COM147/commit48/guard25us,stack4128/finaloff/restoredARR6399.
IRQunion67.881809%, not totalCPU; comparator15479calls/s versus10k~10312/s.
Notan improvement atthis point. Nextone carrier24_hold62_01 at6.2%10s,
sameother settings/allguards, still existingfirmwaredutycap. Observe speed/
jitter/IRQcost before making any carrier verdict or expanding commands.

carrier24_hold62_01 PASS10s278.837228eHz,sigma58.944872us,16731COM/16730acc,
raw268/bus11128mV,arm56us/cost14,COMP87/COM199/commit48/guard25us,
ADC49751/DMA18queue2,stack4120,finaloff. IRQunion68.538825%.
Thus no demonstrated24k benefit in254..279eHz regime: moreCOMPcalls and
IRQwalloccupancy, higher cycle sigma, and longerpreemptibleCOMbrackets.
COM199us isNOTtheatomiccommit48us; don't conflate either with CPU utilization.
No24krecoverycohort. Next investigatequalification/IRQtiming versusreference
beforemorehigher-duty runs. Keep10kA54DCFEA asqualifiedcomparison, not claim
nominalreferencecarrier guarantees reference behavior onthisMCU/adapter.

## E295 comparator adapter inlining experiment

Frozenminz/core/src/am32_isr.rs/live hashboth
54b9f4957e69ad3606cc4db90397d2071a536f39ecceb320c54e4768a8c92ab2.
Filter is consecutive agreeing reads, abortonfirstdisagreement, NOT averaging.
3DEAC2F0 disassembly had CachedComp::output_level outofline at0800462c,
calledfrompersistence loop. bench-inline-comp adds inline(always) onlyto
CachedComp::output_level andread_comp_level. No samplecount/polarity/signal
caching/guard/core changes. Faster reads change the wall-time aperture, so
this is a measured experiment, not free semantic equivalence in analog time.
Candidate945D67886B499176CBD199ABD4D9595E25E0C097B5FADBA9460FF7A9B908A624
release/s/thinLTOtext119748/data1088/bss29264. Disassembly hasnooutlined
output_level symbol; comp_isr loop08004d88..08004e02 retainsfreshCSRload
08004d94 eachiteration, agreementcheck/backedge, countbound fromfilter_level.
HostCOMPREAD markervalidatedand--inline-comprequiresit.

Gate beforepoweredexperiment: exactdisabledrole2666/restorecheck,cpuoverhead
max<=10us andarchive3x3/fullfinaloff. Thenone inline24_hold58_01 at5.8%10s,
same24k/phase60/coretrace0/6.2startup/allE294numericguards andfullverifier,
requiredCOMPREAD/PWMROLES/ADC2666metadata. Compareagainstcarrier24_hold58_01
atunchangedsettings; no actualaperture/runtimeclaimfromdisassemblyalone.

945D6788 installed,UARTworkedafterresetwithoutclockrepair. inline24_disabled_01
all6flags127/12..13us/restoreoff;cpu_01 max2/7/10;archive_01 3x3/finaloff.
250Pythonpass. inline24_hold58_01 PASS10s253.873708eHz15233COM/15232acc,
sigma43.497474us,raw228/bus10996,arm57.5us/cost14,COMP81/COM135/commit48/
guard25us,ADC49751/DMA18queue2,stack4128/finaloff. IRQunion66.492650%.
Comparedsamecarrier/duty E294254.266759eHz/sigma58.848151/COMP87/COM147/
IRQunion67.881809: promisingfirstimprovement, not a repeatedcohort orproof
of the near300eHz fault'scause. Candidatechangesreal-timepersistenceaperture
withoutchangingcount; keepthatdistinction. Nexthigher6.2pointandlonger
recoveryqualification before treatingthisasreliableportableperformancewin.

## E296 next gate

One inline24_hold62_01, sameinstalled945D6788,10s6.2%powered duty,
phase60/coretrace0/24k/unchanged6.2startup. RequireCOMPREAD/PWMROLES24006/
ADC2666 andallE294electrical/age/cycle/arm/commit/deadline/finaloff checks.
No limit changes. Compareagainst E294carrier24_hold62_01 beforedeciding
whether to runone30soriginalbudget injectedtracking recovery atthispoint.

inline24_hold62_01 PASS10s278.803878eHz,sigma43.982691us,16729COM/16728acc,
raw288/bus11008,arm41.5us/cost13,stack4064/finaloff. Same-speedprevious
carrier24_hold62_01 sigma58.944872us, so improvementappearsatbothdutypoints.
Nextone inline24_reentry62_30s_01:30s ORIGINALbudget,dropoutat2s,ONEfresh
recovery,same6.2/24k/phase60/coretrace0/allguards. Requireactualinjection,
FIRSTSEG,freshseed/actualarm>=32us/cost<=16,remainingwindowcompletion,
originaldeadline/fullcapture/finaloff. No retryorlimitrelaxation onrefusal.

inline24_reentry62_30s_01 PASS, resumed27.989126s279.001593eHz,
46854COM/46854acc,sigma40.476702us,raw265/bus11032,seed1216,
originaldeadline208usspare,stack3992/finaloff. Declaretwo confirmations
inline24_reentry62_30s_02 and_03 atEXACTsame settings/gates, stopserieson
firstfailure; retainalldenominators. No newduty/carrier/firmwarechange.

Confirmations02/03PASS: recovered279.323196/279.048555eHz,
sigma40.828743/40.246768us,arms41us/cost14,deadline189/131usspare.
Fullcohort3/3startup-to-originaldeadline,3/3injectedrecoveries; allstack3992,
COMP81COM135commit48,IRQunion67.16..67.37%,finaloff.
captures/inline24_reentry62_cohort.csv retainssettings/build/hashes.
Nextduty-ledexpansion: firmware/fixture cap6.2 presentlycouplesdriven
acquisition duty to BEMFduty. Decouplecommands/provenance while preserving
qualifiedstartup/current/timing/ageguards; don'tcallthiscommandcap a motor
ceiling or blindlyraise acquisitiondrive to reachhigherclosedloopduty.

## Source-grounded difference

Frozen archive `captures/reference/minz_20260912.zip` has SHA256
`2799b284bca75ff97e7d5a8cb57eae4ba0ce32f581e80203f6bc7168e9606c44`.
Its `minz/src/tim1_motor_pwm.rs` matches the inspected sibling file byte-for-byte,
SHA256 `cb9f06a60308c80374212d985c8b45466f6044a04bbc030277607e9a245a79af`.
This is archived source evidence, not a new reference-board experiment.

| Operation | Current binz shell | Frozen reference |
| --- | --- | --- |
| Duty | Source CCR nonzero; other CCRs zero | Equal values in all three CCRs |
| Commutation | MOE clear, CCR/mode/enable rewrites, EGR.UG, mux, MOE enable | GPIO role changes; no CCR writes or UG |
| Floating/low legs | Timer output modes/CCER | GPIO output-low / low-input output-high |
| Carrier | Restarted each commutation | No counter restart in role change |

Cached STM32G071 PAC `tim1/egr.rs` explicitly documents UG as counter
reinitialization plus register update. Thus resetting the carrier is an actual
implementation difference, not inferred from ADC phase-bin histograms.
The reference also documents why per-phase preloaded CCRs can create a stale
zero-duty window. Removing binz's UG alone is therefore not an acceptable fix.
Carrier reset plus output blanking can affect pulse timing/area; these sources
do not establish that it caused the observed5.5% CycleTiming stops or even the
sign of the average-voltage change. Do not infer delivered duty from a simple
reset-count model that omits MOE blanking/deadtime.

## Current-board geometry and safety contract

`examples/support/phase_gpio_plan.rs` preserves the qualified binz logical
sequence and pin map, not the reference's numeric phase/sector labels:
A=PA10/PB1, B=PA9/PB0, C=PA8/PA7. Source uses complementary AF; sink has
high input GPIO-low and low input GPIO-high; floating has both GPIO-low.
Tests cover all six sectors, explicit A/C pin cases, unrelated-pin preservation,
nonconflicting BSRR writes and invalid-sector refusal. No peripheral writes.

Live integration still must satisfy all of these before a powered experiment:

1. Guard/ownership check and role write stay in one bounded critical section.
   Clear MOE and all gate GPIO latches on **both ports** before setting the new
   sink. This preserves binz's break-before-make policy; do not copy the
   reference's keep-MOE-set policy blindly.
2. Configure all three complementary PWM channels and equal nonzero CCRs while
   outputs are safely disabled. Ensure active compares are loaded before first
   enable. Initial UG is allowed; ongoing role changes must not clear CCRs or
   restart CNT. Keep duty updates separate from role-only commutations.
3. Global stop/panic/guard still clear GPIO lows, MOE, CCRs and ENABLE. Clearing
   only MOE is insufficient once a sink is GPIO-driven. Reset preparation state
   on every safing path; restoration must never resurrect an old role.
4. Sine/forced-drive entry must explicitly restore AF/mode assumptions. Keep the
   proven comparator polarity, floating-phase mux, deadtime and duty guards.
5. Add explicit capture provenance and disabled register/pin/counter checks.
   Then qualify guarded low-duty startup/hold/current/timing before recovery or
   duty expansion. Different waveform geometry may change speed at equal duty;
   retain existing speed/age/arm/electrical refusals and all failed attempts.

This is a bounded experiment toward the existing duty-led goal, not a new
performance ceiling, calibration shortcut, or reason to switch to AM32 now.
