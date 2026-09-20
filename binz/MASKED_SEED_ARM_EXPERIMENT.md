# Fresh-seed masked comparator preparation — E487

Current status: E488 candidate C631 is installed, outputs verified OFF, and one
matched recovery campaign passed. See E488 below; E487 notes are historical.
The E486 running-cycle failure remains unresolved.

Latest69recovery CORESEED still reports170half-us=85us edge age, despite7us
arm-body cost. Thus old85us budget table remains relevant: minimum350-profile
seed952 gives119us wait,34us remaining,only2us above32usfloor. This does not
explain E486's running cycle refusal. Neither guard is changed in this work.

Source found a concrete redundant sequence inside the globally masked fresh
seed arm block: observe_irq_start configures sense mux/priorities then enables
COMP; caller immediately calls Comp.mask_interrupts then clear_pending before
sampling age/arming bootstrapCOM. No COMP ISR can execute between these calls.

Candidate feature bench-masked-seed-arm uses a const-specialized setup helper
that keeps comparator hardware delivery masked throughout. change_input already
masks hardware; explicitly set MASKED=true too because Input's mask does NOT
update Comp's software latch. Preserve all existing pending clears, active
state, TIM16 unmask, priorities, seed identity, live age and safety checks.
Generic observe_irq_start uses false specialization, retaining its behavior.
First real bootstrap COM retains normal comparator enable responsibility.
This removes enable/remask writes, not the filter or any guard. There is no
measured latency gain yet, nor a claimed steady-state CycleTiming fix.

Initial build2779 lacked explicitsoftware latch assignment; source review caught
and corrected it BEFORE any flash. Finalrelease-s/thinLTO candidate:
BEA2B6A9ABD9362F02D101ACDF28253C57C28668575185727CC14DFA5BB616E5.
Features: samelean20k9ACC profile plusbench-masked-seed-arm. RootELFBEA2;
frozenreference/carrier20_9acc remains the measured baseline. No sharedminz
source edit. Build succeeds, but not proof of pending/mask/arm correctness.

Before power: add fixture provenance and ENABLE-low actualregister checks for
software/hardware masks, no unintended pendingCOM, correctbootstrap enable,
and restored disabled state. Compare emitted setup sequence with baseline;
test earlyrefusal/cleanup and retain ordinary guard/CPU/role/ADC/archive checks.
Then matched69recovery with unchanged350limits; compare actualSEEDLAT/CORESEED
age,armcost,stack and originaldeadline. Do not claim savings by moving the
timestamp later or expanding the profile ahead of measured results.

## E488 — disabled qualification and first matched recovery

Release `s`, thin LTO, one codegen unit; installed/root ELF SHA256:
`C6311D58B44A797252C5F89479A74EFA95FF947252F6999478389E30A884C5B2`.
Same lean 20 kHz / range350 features as 9ACC plus `bench-masked-seed-arm`.
No shared minz source change. OpenOCD reset after download succeeded without
the UART clock repair. UART is closed and outputs are verified off.

Added idle-only `seedmaskcheck` and strict fixture verifier. Three trials of
all six sectors pass with a deliberately stale software-unmasked latch and a
pending NVIC comparator request. Setup leaves software/hardware/NVIC delivery
masked and COM stopped. The ordinary enable primitive works, followed by
disabled cleanup. This runs under PRIMASK without gate authority: it does NOT
exercise a real bootstrap COM ISR, powered preemption, or every early refusal.
The real late-seed/over-budget branches still call observe_end/gates_off; this
is source-reviewed cleanup, not new fault-injection coverage.

Evidence:

- `captures/seedmask_488_guard01.txt`: three timer faults / 18 post-stop refusals,
  SHA256 `3E2A9A41526C36A8D056E75EF5D5A0006ABD0A277DD3DCC3CFE1AA4CA4F5E0FE`.
- `captures/seedmask_488_check01.txt`: register trials pass, SHA256
  `25EA80EEA322E0A9073506CF3F04057C39FA70E7E49D24279595E4977363DE1D`.
- `captures/seedmask_488_preflight01_{pulse,atomic,roles,cpu,archive}.txt`:
  all pass, CPU maxima 2/6/9 us against unchanged gates.
- `captures/seedmask_488_adcroute01.txt`: three disabled route checks pass.
- 398 Python tests and 208 library + 34 other Rust tests pass. Existing Windows
  incremental-cache access warnings did not fail the Rust tests.

The linked `.math-audit.json` SHA matches C631. Emitted true specialization at
0x08008188 calls change_input, clears pending, stores MASKED=true, configures
priorities and unmasks TIM16 only. The fresh-seed caller at 0x08005fc0 calls it,
retains pending clears, then reads the actual timer; no COMP enable/remask or
arithmetic helper appears in that setup bracket. This is a local codegen check,
not a claim that the entire ELF lacks soft arithmetic (the advisory audit still
lists existing helper sites).

One matched campaign, no retries or excluded failures:
`captures/seedmask_488_start61_reentry69_30s_01.txt`, SHA256
`B3B9AC23D18612A792829D181463383C9855DB8084994DBFEA78F19A6F90BE52`.
6.1% driven startup, +60-degree alignment, 6.9% BEMF duty, 20 kHz carrier,
30-second original budget, injected loss and one fresh-seed recovery.
Existing electrical, tracking, range350 and 32-us arm-floor guards unchanged.

Resumed 27.990642 s at 333.790923 eHz, cycle sigma21.938237 us,
56058 COM / 56057 accepted, IRQ-union50.466855%, COMPmax46 us, COMmax45 us,
commitmax21 us, raw peak350, busmin10781 mV, untouched stack2668 bytes.
Original deadline, separate archive, trigger-only ADC3200 CRC and final-off
checks pass. Raw current is NOT calibrated amperes; accepted timing is not an
independent rotor measurement or full parity proof.

SEEDLAT = 88/106/120/152/154 half-us; CORESEED age166 half-us (83 us),
interval1000, remaining ARR84, arm body7 us. Baseline E485 age170 half-us
(85 us), arm body7 us. Observed setup gain is 2 us in one run, not WCET.
At hypothetical minimum seed952 this age would leave36 us versus34 us before,
but that arithmetic is NOT a demonstrated operating point or permission to
raise the speed guard. Running-cycle outliers are a separate open mechanism.
