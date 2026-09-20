# Atomic backend experiment — E300

## Evidence and scope

Installed E299 FDD9319B uses portable-atomic1.15.0 critical-section backend.
Disassembly at08004e26..08004e2e masks/restores PRIMASK around the reference
average_interval load, and similarly around filter/rising and many shell loads.
Cached library README lines122..186 explicitly says this backend takes critical
sections for all operations, while unsafe-assume-single-core avoids masking for
pointer-width-or-smaller loads/stores. Its interrupt/README confirms this rule.

Cargo feature tree found TWO forced enables: root manifest and upstream HAL.
minz-core does not force a backend. Local HAL checkout was clean at
15aca632a5f8ff4bacfad88d254b5a54a64480e1, matching the prior git dependency.
Only its Cargo.toml portable-atomic dependency changed:

```diff
-portable-atomic = { version = "1.10.0", features = ["critical-section"] }
+portable-atomic = { version = "1.10.0" }
```

Root patches the upstream URL to ref/stm32g0xx-hal. That checkout is ignored:
recreating a workspace requires that exact checkout and the above one-line patch.
No library implementation or sibling minz code changed. Cargo.lock now points
to the local HAL. Default root feature atomic-critical-section retains the old
backend. Experiment requires --no-default-features --features
bench-single-core-atomics plus the E299 campaign features. Enabling both backend
features is rejected by portable-atomic, not silently resolved.

## Safety requirements before flashing

This is specific to the single-core G071 running privileged bare metal. Library
RMW/CAS operations still disable interrupts; explicit multi-field critical
sections remain untouched. No CONTROL/nPRIV transition or custom NMI/HardFault
state-mutating handler was found in shell/support/src. Do not generalize this
feature to multicore or unprivileged execution. PRIMASK does not mask NMI or
HardFault; neither the old backend nor the new one provides synchronization with
an independently mutating unmaskable handler. DMA must not own atomic state.

Before powered use add fixture backend provenance and an outputs-disabled
atomic semantics diagnostic (load/store/swap/CAS, nested PRIMASK preservation).
Audit DMA destinations and startup privilege assumptions; then run existing
role, CPU-meter, archive and recorder-refusal preflights. Keep electrical,
tracking, storm, arm and finite-deadline guards unchanged. Requalify at the
existing point first; shorter physical persistence aperture can change sensing.

## Offline result — not installed

Release/s/thinLTO/codegen1 candidate:
9223AA9687EDBF96F4266AC1318B660383A792FA0DE41F25ED3A425271C80449.
text110520/data1128/bss29260 versus120256/1120/29264 installed.
Code shrank9736bytes; this is NOT a CPU percentage prediction.
New comp_isr080041bc reads average at08004210 directly, filter0800422e,
rising08004234 without per-load PRIMASK transactions. Feature tree confirms
unsafe-assume-single-core and absence of portable-atomic/critical-section.

No flash, UART or motor action E300. Installed FDD9319B remains last-off E299.
Next work is the bounded safety/provenance checks above, not duty expansion.

## E301 disabled hardware checks

Added idle atomiccheck with256 repetitions of load/store/swap/CAS success and
failure/fetch_add, both outside and inside nested explicit critical sections.
Checks PRIMASK restoration and privileged CONTROL state; test data is local,
not DMA-owned. This is semantics/nesting evidence, not concurrent-ISR stress.
Active DMA destination audit finds owned ADC ring, PWM sample buffers and phase
timestamp ring, not the controller Atomic fields; transport/probe buffers are
separate feature paths. Existing DMA ownership/fences are unchanged.

Installed E0FE22777F94C78C05F89D664EE7F54F69ADCD2DBA93C1830CFDF93885006C67:
text111316/data1128/bss29260, release/s/thinLTO/codegen1. ATOMICBACKEND emitted
in fixture dump, required via --single-core-atomics.258Python tests passed.
Initial atomic01 and roles01 UARTsilent captures retained. Exact hardware
RCC08000000,PD1ODR0,BDTRc1a,CCRs0 confirmed before knownUARTclock08040000
repair. atomic02 passes256,privileged1,entry_unmasked1,restored1;
roles02 sixflags127/12us, CPU01 max2/7/10us, archive01 three3/3checks.
Finaloff verified, ports closed. No motor run yet. Finish recordcheck before
the first bounded powered comparison; no runtime speedup claimed.

E302 predeclared comparison: recordcheck12/12 refusals and unchanged records
passed. One10s6.2%/phase60/trace0/24k hold with paths and single-core markers
required. Require complete existing verifier, bus>=8400mV,raw<=1200,
actualarm>=32us,cost<=16us, ADC/tracking/deadline/CRC and finaloff. Stop expansion
on failure. Compare E299 same instrumented setting; no guard or duty change.

E302 hold passed279.45655eHz, IRQ62.47127% vsE29970.12571%, sigma39.7378
vs51.1467us; COMP75/COM105/commit46us, raw280,bus11032,arm65.5us/cost12,
stack4076, finaloff. Visits178131 (more than old145198), not lower traffic.
Predeclare next ONE30s original-budget dropout/reentry, same6.2/phase60/trace0/
24k/backend, full existing recovery/guard/seed/arm/deadline and finaloff gates.
No higher duty; any failure stops escalation. Single-run gain not reliability.

Recovery also passed: singlecore_reentry62_30s_01, original30s budget,
27.98931s resumed at280.03645eHz,47028COM/accepted,sigma35.18649us,
IRQ62.79455%,COMP75/COM105/commit46us,raw280,bus10972mV,stack3552,
freshseed1211/acquisition7833us,actualarm53.5us,deadline242usspare.
Counter reset verified:498017dispatched=87028closed+363961open-noaccept+
47028accepted. Full fixture/finaloff pass. Same installedE0FE2277,portsclosed.
This establishes n1hold+n1recovery, not whole-envelope reliability or total CPU
utilization. Instrumented old same-duty hold70.12571% vsnew62.47127% is7.65444
percentage points less IRQ union despite more COMP visits. Do not project this
linearly to throttle. Next normal-build overhead removal/repeat qualification.

E303 fixed confirmation cohort: exactly TWO additional30s dropout/reentry
attempts on installedE0FE2277,6.2%/phase60/trace0/24k, unchanged counters and
guards. Stop on first failure; retain all attempted raw captures. Together with
E302 this is a three-attempt cohort, not retries-until-pass. Require backend and
path provenance, complete original-budget recovery verification, bus>=8400mV,
raw<=1200,actualarm>=32us,cost<=16us,tracking/ADC/deadline/CRC/finaloff.

E304 removes only bench-comp-paths from E301features, release/s/thinLTO.
Installed1E13B559CEE7566B3DA36C593D52D075267E90B218B42B80557F0A8C719321E6,
text110740/data1104/bss29260. atomic256/roles6/CPU2,7,10/archive3x3 passed,
UART responded without clock repair. PredeclareONE10s hold6.2%/phase60/trace0/
24k/single-core backend. Same bus>=8400/raw<=1200/arm>=32/cost<=16 and full
tracking/ADC/deadline/CRC/finaloff gates. COMPPATH must be absent; no duty change.

E304 result singlelean_hold62_01 PASS280.562024eHz,16834COM/16833accepted,
sigma39.503170us,IRQ58.471589%,COMP72/COM98/commit46us,arm68us/cost12,
raw272,bus10996,stack4124,DMA18queue2. COMPPATH absent, finaloff verified.
RawSHA256 ab23dfbccd4ec85d21376372094502a1a1f2dedb566adc22d28c5809a5e3d72e.
Single hold comparison vsinstrumented62.47127% shows4.00points lessIRQ union;
not exclusive counter cost because traffic/timing also change. Lean recovery
unqualified; prior3/3 cohort remains scoped to E0FE2277. No duty expansion.
