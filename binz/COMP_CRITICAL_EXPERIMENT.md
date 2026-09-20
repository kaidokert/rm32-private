# Bounded reference COMP-service exclusion - E402, 2026-09-14

## E478 — matched sparse 7.2% exclusion still fails without final overlap

Audited priority proposal: IRQ_RATE64/1ms is not a guard deadline. Added pure
limiter regression demonstrating20us-spaced calls run100ms through16bit wraps
at50/ms without refusal; even5us hits can pass210us before65th. Synthetic
counterexample, not measured ISR occupancy/NVIC simulation. Global priority
swap therefore cannot claim guard delay bounded by one call.208lib+34RustPASS.
Reused existing per-call critical probe instead, guard priority0 unchanged.

Frozen E476 baseline captures/reference/qualsparse_0f9f/shell-pwm.elf hash
0F9F8A9F6E5BB021045A91AD864762BCBC8E937AF1134C79DA99ABC692819AFD.
Candidate adds bench-comp-critical to same sparse/350/24k feature profile:
F22D53B01247D5F2D4257E1FCF62E379D1386BDA547B8AF954B1A1F69DDF74AC.
Releaseopt-s/thinLTO automatic disassembly audit hash matches. Emitted wrapper
PRIMASKsave080041ca/CPSID41ce, referencecall41da, timerend41de, restore41e0;
filter12 admission and postrestore60us refusal confirmed. Reference volatile
read loop remains within call. Not a WCET proof or pure priority-only A/B.

Installed afterverifiedoff/OpenOCDreset. qualsparse_critical478_guard01PASS3/18;
scope01PASS2/4/4/4/1/4/4/4+wire; wrapper01PASS1/8/7/69us (69 deliberately
overrun synthetic case, not allowed live cost); fullfivepreflight01PASSCPU2/6/9.

One matched30s72BEMF/61startup/phase60/200eHz initialhandoff/24006carrier
dropout/reentry capture FAILED, preserved:
captures/qualsparse_critical478_start61_reentry72_30s_01.txt
SHA256 BA320E858D58EF41B32F4E9F8237E030F090CEDF854E648603338B074548C6C8.
Recovered then CycleTiming12 at1.724832s resumed;3482COM/3481accepted,
336.448833eHz aggregate,sigma29.367425us,rawpeak259,busmin11080mV,
stackuntouched2244. External11.7V/800mA setting assumedunchanged, notremeasured.
StrictCOMPCRITICAL31060calls/max45us/refused0; finaloffverified/UARTclosed.

Sparseepoch134 boundvalid:16rows/1105selectedomissions. Final51us step5,
accepted_before3481,notlogaccepted,stopped,NOguardoverlap. CYCLECORE proves
reference persistencepassed. Guarddelta2856<2858, referencecycle2852.5,
closure3.5us. Previous3090.5 vs local2993 gives+97.5/-140.5us pair,-21.5mean.
The same fault class exists with reference service excluded and finalcall
not overlapped: midservice guard preemption is NOT necessary for this outcome.
Does not eliminate pre-entry delay, earlier-call effects, PWM qualification
timing, or prove hardware/physicaloverspeed. Sparse selected workload is much
larger under wrapper; no fair occupancy/jitter gain conclusion from this A/B.
Next inspect pre-entry/acceptance timing and qualification semantics outside
this intervention. Do not repeat broader exclusion hoping for a pass, widen
guards, or promote candidate to reliable operating firmware.

## E404 powered comparison: preceding point passes, 6.9% still fails

Actual044F unchanged. Disassembly verifies PRIMASK save/CPSID at08003f52/56,
reference call08003fe0, timer end08003fe4 and restore08003fe6. Reference
12-read volatile loop080046cc..46e8 and recorder call080047a4 are inside;
argument setup is also inside. Post-restoration duration check remains outside.
Wrapper local stack92, reference20, recorder44 bytes, plus register saves and
other callees: these are not a total stack/WCET proof. No UART/ADC wait added.

compcritical_hold68_01 PASS10.000032s,315.798eHz,18948COM/18947accepted,
raw293bus10877,body188177calls/max32us/refused0,stack3328.
compcritical_reentry68_30s_01 PASS original30s,27.990539s resumed315.862790eHz,
53047COM/53047accepted,sigma28.461115us,IRQ59.033705%,raw254bus10996,
body526744calls/max33/refused0,COM49/guard20/commit21/DMA21us,queue2,
stack2676,arm48us/cost12 (16us above floor),SEEDLAT86us. All strict
provenance/timeline/finaloff checks passed. Occupancy rose from prior DMApeer
53.135%; protection is not free and this one pass is not causal proof.

Then compcritical_reentry69_30s_01 FAILED before planned dropout/recovery:
CycleTiming12 at159790us,step2 previous156754->159783=3029us below3031floor.
293COM/292accepted,raw233bus11343,body3048calls/max48/refused0,stack2676.
Reference adjacent cycles6386/6053 half-us ticks,pairedmean3109.75us;
previous recorder25us after guard. Not an independent rotor-speed measurement.
Recordguard late_accepts1 reflects the fault-path recorder; finaloff verified.
Fixture exited1, no retry/higher duty/guard relaxation. Protection does not
eliminate the failure; neither a sole preemption cause nor hardware cause is
established. Next compare initial transient and acceptance/dispatch semantics
offline. Current044F remains diagnostic, not qualified6.9%.

SHA256 captures, in order above:
E10E60910462BA4D04471D4833E92B2671F9A56D05FE17A12859CC5745C16D8D
4832279B505F7068883D8BBBB6B4D70DFC5F163FD9CD98064F37557F3FAC8A4F
69C89E6DC12D72DA39AC1A267373C1985BA711FA3615A43689A021661F3E57EA

Older entries below describe their state at the time, not current installation.

## E403 installed diagnostic, wrapper tests passed (no motor)

Actual044FF992156A607940B1B49BFAC29D6A22DB76085C768F050F55CDDEB3659D04,
release-s/thinLTO/autoauditPASS,text125128/data1104/bss29344. Shared board-local
critical_service helper handles filter admission/timer bracket/RAII restore.
Strict --comp-critical requires used/nonrefused/max<=60 marker.337PythonPASS
before hardware; baselineA2DE archived/hashverifiedreference/dmapeer_a2de.

Disabledcheck exercises actualhelper with earlyreturn,12volatile reads,
nestedPRIMASK and intentional61usdelay,16trials each; invalidfilters0/13
must neverexecute closure. wrapper01UARTsilent; exactRCC08000000/PD1zero/
BDTRc1a/CCRs0 thenknownclockrepair. wrapper02PASS maxima1/7/6/68us,
restoration/all16trials valid. Lastmode tests overrun detection, not a passing
production duration. It does not invoke productionaccepted-recorder workload.
guard01threefaults/18refusals and preflight01fivechecksPASS CPU2/7/10.
Finaloffverified/portclosed. No motor run on this image yet. Next inspect
emitted fullservice mask scope/callgraph/stack, then bounded preceding6.8%
qualification; don't claim this wrapper-only test establishes service WCET.

Optional bench-comp-critical staged, NOT flashed or qualified. ActualA2DE
remains installed, last motor E401 offverified. No shared minz source edits.

The reference has no separate qualification hook: am32_isr99..108 reads live
samples,109 starts mask/reset/COMarm critical section, then EV_ACC is recorded
outside it. Protecting only the read loop would leave another preemption gap
between qualification and acceptance. Changing that seam would modify shared
reference code. This experiment instead wraps ONE existing comp_isr call on
the specialized real/inverted/traceoff powered path, including accepted-event
recording. It is explicitly NOT a read-loop-only mask or a whole ISR/campaign
mask. Dispatch/rate checks and higher-priority guard service between calls
remain outside. All original control operations are called unchanged.

Only exactfilter12 may enter; another filter value records a refusal and
aborts. The existing core path has bounded software operations, not ADC waits
or UART formatting. A TIM17 bracket inside the critical section measures the
body; max and refusal counters update after PRIMASK restoration. Above60us
adds an abort (existing HostAbort route), with explicit COMPCRITICAL refusal.
This is a POSTHOC overrun check, NOT an independent masked-code watchdog.
The counter bracket excludes critical-section prologue/epilogue instructions.

60us is an experiment gate, not an operator limit or WCET claim. With the
100us guard period and200us tick-gap guard it leaves nominal40us for other
delay; that arithmetic alone does not prove scheduling safety. Generated-code
and disabled worst-path measurements are required before power. The guard
stays priority0 and may run when PRIMASK restores between successive calls;
unlike globally lowering its priority, pending service is not indefinitely
subordinated to an entire COMP IRQ stream. Existing global mask costs remain.

Counters reset at observation_reset (including recovery). Post-stop fixture
COMPCRITICAL reports calls/max/refused,filter12,limit60,recorder_inside1 and
restores_between_calls1. Diagnostic/trace-on fallback is unchanged and not
covered: strict host expectation must reject a missing/non-used probe path.

Next: strict capture checks plus disabled test of the actual wrapper's early
reject/accept/stop paths, PRIMASK restoration and measured mask duration.
CPUCHECK alone does NOT exercise this new wrapper. Verify emitted scope/stack,
retain baseline artifact, then normal guard preflights and matched6.8% tests.
No6.9% experiment until preceding qualification. Never relax timing/current/
age guards or infer an outlier cause merely from a favorable new run.
