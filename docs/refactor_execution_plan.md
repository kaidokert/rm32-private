# Generic-core refactor: step-by-step execution and regression plan

Companion to `docs/generic_core_design.md` (r3). That doc says WHAT changes;
this one says in what order, how each step is proven not to regress the
qualified results, and when the battery comes out.

Reference bench (operator decision, 2026-10-25): **both boards on the shared
5 A PSU**. Battery only for steps classed RISKY (section 4).

| Board | MCU | Probe | Bench UART | Qualified results to protect |
|---|---|---|---|---|
| Vimdrones L431 | STM32L431 @ 80 MHz | `0483:374f:0037002F3234510836303532` | COM18 (S = PA2 in, TX1 = PB6 out), 2 Mbaud | 2026-08 envelope 15-100 % (bf_ladder / bf_slam via BF). Today's FC-free state: 10-30 % holds clean (2026-10-24, `7cd3dfe`). |
| binz G071 | STM32G071 @ 64 MHz | `0483:374b:066CFF343433464757233430` | COM41, 115200 | 100 % on 3S battery (`765fcf9a`), 5→100 slams (`5565a34c`); 80 % top rung on the 5 A PSU (`167b7ae9`). |

## 1. Rules for the whole campaign

1. **One step = one commit = one tag.** Each step is small enough that a
   regression bisects to it without further bisection. Tag every passing
   step `gc-sNN-ok`; push to `private/devel` after it passes.
2. **Both images from the same snapshot.** Build L431 and G071 from the
   committed tree, record both ELF SHA-256s in the step log, flash both,
   run both. Never test an uncommitted tree.
3. **A failure is a code cause.** Flash the previous `gc-sNN-ok` images,
   reproduce the pass, then bisect. "Bench drift" is not an explanation
   unless the operator names a physical fault.
4. **The harness moves with the core.** Every step that changes a core
   interface updates `rm32/src/bin/harness.rs` (later `rm32-harness`) in
   the same commit; vectors are green at every commit.
5. **Bench discipline** (unchanged from the campaigns): kill on every exit
   path; one hold per command; no UART printing while the bridge drives;
   no commits while a ladder is running; "pause" means stop after the
   current run.
6. **Disassembly is evidence.** Steps that should not change control code
   are proven by an empty ISR-root disassembly diff, not by a bench run
   alone.

## 2. Phase S0: preparation (before any refactor commit)

These make the reference measurable. None of them touch control code.

| Step | What | Done when |
|---|---|---|
| S0.1 | Commit `generic_core_design.md`, the review, and this plan. | Pushed. |
| S0.2 | **PSU guard profile for binz.** binz is hardwired to `BINZ_BATTERY` (`main.rs:486`): 8 A / 15 A surge / 17.5 % sag. On the 5 A PSU the supply folds before the current guard trips. Add a build-time supply selection (`BENCH_SUPPLY=psu|battery`) that picks `BINZ_BRINGUP` (4.5 A sustained, 10 % sag) for PSU images. The control code is identical between the two; only guard constants differ, and the step log records which one was flashed. | Both binz images build; ISR-root disassembly identical apart from the guard constants. |
| S0.3 | **L431 guard on the PSU.** The L431 auto-profile classifies the supply by rest voltage (> 10.5 V = battery), so the ~11.7 V PSU gets the battery limits (15 A). Add the same `BENCH_SUPPLY=psu|battery` selection: PSU = fixed 4.5 A sustained (operator decision, section 8). | PSU image kills at 4.5 A sustained (host test of the guard) and runs the 10-30 % profile clean. |
| S0.4 | **Power path check.** Measure sag vs current on the spliced PSU feed (both boards share it) at idle and at a steady ~3 A hold. Memory: the bench power path has melted before at ~0.65 Ω. | Path resistance logged; below 0.3 Ω or fixed first. |
| S0.5 | **L431 recorder.** Port `rm32::bench_rec` (R / Y / X keys) to the L431 benchuart build and teach `binz_profile.py` (or a shared `profile.py`) to drive either board by port/baud. Without this the L431 has only `i`-line polling, and the two boards cannot be compared on the same quantities. Bench-only code, behind `benchuart`. | 10 → 30 % profile produces a CSV with duty / eHz / mA / mV / old_routine / dsy on the L431. |
| S0.6 | **Gate tooling in one command.** A `scripts/gc_gate.sh` that runs: `cargo test -p rm32`; blackbox (75 vectors, Rust vs C harness); both cross-builds; M0 link audit; `binz_wcet.py`; flash/RAM size report for all four MCU builds; ISR-root disassembly dump (L431 + G071) into `captures/gc/<step>/`. | One command produces a pass/fail and the artifacts. |
| S0.7 | **Disassembly diff tool.** Compares the ISR roots (TIM6, TIM14/TIM16, COMP, DMA, EXTI handlers and everything they inline) between two steps, ignoring addresses. | Diff of a commit against itself is empty. |

## 3. Phase S1: the reference baseline (= design phase 0 / issue #7, PSU edition)

Build both images from the S0 tip, tag `gc-s00-baseline`, then run the
**reference suite** three times per board, interleaved L431 / binz / L431 /
binz ..., same session.

### Reference suite (PSU)

| ID | Profile | Purpose |
|---|---|---|
| R1 | Arm, walk-in, ladder 10 → top in 10 % rungs, 5 s each, slow recorder | Retention and speed/current curve |
| R2 | Start from arm to 10 %, three times | Start reliability |
| R3 | Step 10 → 60 % up, then walk down 40 / 25 / 15 % at 0.35 s, 10 % 4 s (fast recorder) | Transient: lock through a big up-step and the decel |
| R4 | Hold the PSU top rung 15 s | Steady-state thermal/current at the supply limit |
| R5 | binz only: coast-down `C` at 50 % | True rotor speed vs loop estimate |
| R6 | Idle register snapshot (L431 `dump_l431_regs.py`, binz `binz_regdump.py`) | Peripheral configuration parity |

**As run (2026-10-25, fixed for the campaign).** Exact `binz_profile.py`
profiles — later steps must use these strings so segment counts match the
bands:

| ID | L431 (`--board l431`) | binz (`--board binz`) |
|---|---|---|
| R1 | `--rec slow 10:5,20:5,30:5,40:5,50:5,60:5` | `--rec slow 10:5,20:5,30:5,40:5,50:5,60:5,70:5` |
| R2 | `--rec fast --from-arm 10:4` | same |
| R3 | `--rec fast 10:3,60:4,40:0.35,25:0.35,15:0.35,10:4` | same |
| R4 | `--rec slow 10:2,20:2,30:2,40:2,50:2,60:15` | `--rec slow 10:2,20:2,30:2,40:2,50:2,60:2,70:15` |
| R5 | — | `binz_coast.py --rung 500 --hold 10` |

Top rungs: L431 60 % (~2.1 A on the prop), binz 70 % (~2.85 A), both well
under the 4.5 A PSU cutoff. R3 changed from the original 10 → 60 → 10 %
step: on the PSU the instant step-down regenerates into a supply that
cannot sink current (19.9 V L431 / 19.5 V binz, both 14 V OV guards
latched). The full step-down moves to the battery batch. The L431 profile
stop also walks down 60/40/25/15 (an abrupt stop from 60 % pumped 17 V).

### What gets recorded per run

Per rung: median and spread of eHz, mA, bus mV; time in old_routine; desync
count; drops; guard events; reboots; for binz also t6mean / t6max / gaplate
/ gaplost. Plus the host gate outputs (WCET numbers, sizes, disassembly).

### Acceptance bands

Derived from the baseline's own three runs, not chosen in advance: for each
metric, the pass band is the baseline min..max widened by 5 % (operator,
2026-10-25), per segment for duty / eHz / mA (`gc_summary.py --bands`,
checked with `gc_summary.py --check docs/gc_bands_s00.json`). Hard gates that need no band: zero reboots, zero guard kills, zero
desyncs where the baseline had zero, every rung reaches Running, WCET not
worse than baseline. The bands are written into the log at the end of S1
and do not move afterwards without the operator agreeing.

**Bands v4 (operator, 2026-10-25, during S2.1).** mA bands are min..max
±max(5 %, 30 mA); built from all 55 s00 runs (6 rounds plus the S2.1
bisect runs); the 0.35 s walk-down segments keep the duty/dsy/kill gates
but no eHz/mA band. Rebuild command: `gc_summary.py --bands
docs/gc_bands_s00.json --ma-floor l431=30 --ma-floor binz=30
--min-settled 1.0 <runs>` (runs are listed in the JSON). Why: the L431
current is one ADC count = 26.7 mA and is software-triggered from the main
loop, so a layout-only change (s00 + 8 jumped-over NOPs) moves the 10 %
reading 54 → 80 mA; binz's 10 % current spreads 117–170 mA across runs
(per-run 3-shunt zero); three runs had under-sampled both.

## 4. Step classes

| Class | Meaning | Bench proof |
|---|---|---|
| **HOST** | No firmware ISR code change: disassembly diff of ISR roots is empty (only names, comments, crate moves, harness). | Host gate + one quick R1 on each board (smoke: the image boots and runs). |
| **CODE** | ISR code changes, but the change is meant to be equivalent (refactor through a new trait, constant folding that moved). | Host gate + full reference suite ×3 per board, ABAB against the previous tag's images in the same session. |
| **RISKY** | Changes commutation, BEMF acceptance, priority-0 ISR timing, or the guard path. | CODE proof on the PSU, then a battery run: binz 5 → 100 % slam and a 100 % hold; L431 to its 2026-08 qualified range if the bench allows. |

If a step labelled HOST shows a non-empty ISR disassembly diff, it is
re-labelled CODE before any bench time is spent, and the diff is explained
in the log.

## 5. The steps

Design-doc phase numbers in brackets. Each line is one commit.

### S2 — bug fixes [phase 1]

| Step | Change | Class |
|---|---|---|
| S2.1 | Saturate `count() as u16` at `isr_logic.rs:294` | CODE (16-bit ports cannot reach the saturating branch; expect folded-away or one extra compare) |
| S2.2 | L431 timer/DWT-backed `delay_micros` (cortex-m 0.7.8 regression) | HOST for control; check tune duration and boot timing |
| S2.3 | Voltage units → centivolts; KISS / EDT / orbit consumers; `edt_voltage_temp.txt` + `edt.rs` tests updated after checking the C harness's EDT input unit | CODE. Orbit trip and guard-adjacent: verify on PSU that `vbat` readings match the meter on both boards |
| S2.4 | Stale doc fixes | HOST |

### S3 — bench code out of the core [phase 2]

| Step | Change | Class |
|---|---|---|
| S3.1 | `rm32-harness` crate (move the harness bin, no behaviour change) | HOST |
| S3.2 | `rm32-bench` crate: `bench_input`, `bench_rec`, `blackbox`, `dump_config` | HOST |
| S3.3 | `DiagSink` policy; remove `skip_remaining` / `inject_com_skips` from the core; `dbg_*` behind it | CODE (bench builds keep the hooks; production ISR roots lose loads, so the diff is non-empty by design and must show ONLY removals) |
| S3.4 | `rm32_stm32` stops enabling `bench-diag` unconditionally | HOST for bench images (they still enable it); size report for production images |

### S4 — clocks and board constants [phase 3]

| Step | Change | Class |
|---|---|---|
| S4.1 | `Platform` trait with clock consts; `MotorHal` implemented through it | HOST (disassembly must be identical) |
| S4.2 | Constants restated via `us_to_ticks` / `ms_to_control_ticks` (one module per commit if large) | HOST, disassembly identical |
| S4.3 | `six_intervals_to_us` / `e_com_us_to_avg_ticks` replace `>> 1` and `/ 3` | HOST, disassembly identical |
| S4.4 | Board constants out of RAM; compile-time ADC multipliers | CODE (divisions removed from the 1 kHz path; readings must match S2.3 to the count) |
| S4.5 | Main-loop-rate counters onto the control tick | CODE (LED blink / 1 s counters only) |

### S5 — Bridge [phase 4]

| Step | Change | Class |
|---|---|---|
| S5.1 | Core `step_legs` const fn; ports build `PLAN` / `ROLES` from it | HOST: the generated tables must be byte-identical to today's |
| S5.2 | `Bridge` trait replaces `PwmOutput` + `PhaseOutput` | CODE |
| S5.3 | Drive-mode decision (`comp_pwm && !old_routine`) into the core | **RISKY** (commutation path) |

### S6 — BEMF [phase 5]

| Step | Change | Class |
|---|---|---|
| S6.1 | `BemfSensor` trait, `BemfPins` to the port, same call sites | CODE |
| S6.2 | Gate average latched by the control tick into core state; `comp_gate.rs` deleted | **RISKY** (the 2026-07-27 regression lives exactly here) |
| S6.3 | `on_bemf_irq` gate logic into the core; L431 / G071 COMP ISRs become one call | **RISKY** (priority-0 path; WCET gate on the COMP root) |

F051 / G431 gaining the gate is a behaviour change for boards we cannot
bench today; it is recorded, not qualified, here.

### S7 — input and dispatch [phase 6]

| Step | Change | Class |
|---|---|---|
| S7.1 | DShot bit layer added (no caller changes) | HOST |
| S7.2 | `CAPTURE_HZ`, `GCR_SHIFT` to `Platform`; DMA-named types renamed | HOST, disassembly identical |
| S7.3 | Transfer dispatch into the core; harness uses the same function (fixes the drift) | CODE |
| S7.4 | Tone, bidir frame selection, CRSF dispatch into the core | CODE |

S7.3-S7.4 change the DShot path, which the benchuart builds do not
exercise. They need the flight controller reconnected for one session:
DShot150/300/600, bidir, EDT and Configurator passthrough on the L431
(`protocol_matrix.py`). The operator decides when; until then S7.3+ stay
on a branch.

### S8 — shared state [phase 7]

| Step | Change | Class |
|---|---|---|
| S8.1 | `SharedComm` split into groups, no default bodies | CODE |
| S8.2 | `SingleWriter` token and `unsafe` `_isr0` | **RISKY** (priority-0 writes) |

### S9 — tuning policy [phase 8]

| Step | Change | Class |
|---|---|---|
| S9.1 | `TuningPolicy` with `QualifiedRigTuning` = today's constants; rigs select it | HOST for both rigs (disassembly identical) |
| S9.2 | Harness enum of two tunings; vectors declare their tuning | HOST |
| S9.3 | `Am32Tuning` builds for the other boards (size report) | HOST |

### S10 — portability proof [phase 9]

`rm32-sim`, time-based vectors, RISC-V cross-build, STM32-name grep gate.
Host only.

## 6. Per-step procedure (the loop)

1. Branch `gc/sNN` from the last `gc-sNN-ok` tag.
2. Make the change; update the harness in the same commit.
3. `scripts/gc_gate.sh` → host gate + artifacts into `captures/gc/sNN/`.
4. Disassembly diff vs the previous tag; confirm the class (section 4).
5. Commit through the hooks (re-commit if fmt rewrites). Record both ELF
   SHA-256s and the class in `docs/refactor_log.md`.
6. Flash both boards from that commit.
7. Bench proof per class on the PSU. CODE / RISKY: ABAB with the previous
   tag's images (A = previous, B = new, three of each per board).
8. RISKY only: tag `gc-sNN-psu`, keep going on the PSU, and add the step
   to the pending battery batch (operator decision: batched). The batch
   session runs each pending step's images in order on the battery;
   a step becomes `gc-sNN-ok` only after its battery run passes.
9. Compare against the S1 bands. Pass → tag `gc-sNN-ok`, push, next step.
   Fail → flash the previous tag, reproduce the pass, bisect inside the
   step (it is one commit, so the bisect is over its hunks), fix, repeat
   from 3.

## 7. Log

`docs/refactor_log.md`, one entry per step: commit, class, ELF hashes,
host gate summary (vectors, WCET per root, sizes), disassembly verdict,
bench runs (files under `captures/`), comparison against the bands,
battery yes/no, verdict, tag. The S1 baseline and its bands are the first
entry.

## 8. Operator decisions (2026-10-25)

| Question | Decision |
|---|---|
| binz current cutoff on the 5 A PSU | **4.5 A** sustained (the `BINZ_BRINGUP` profile), via the S0.2 PSU image |
| L431 current cutoff on the 5 A PSU | **4.5 A**, as a fixed PSU profile like binz's, replacing the voltage-based battery guess (which reads the ~11.7 V PSU as a battery and would only cut at 15 A) |
| Flight controller session for the DShot steps | **When S7.3 is reached.** S7.3+ wait on a branch until then; I ping the operator |
| Battery runs for RISKY steps | **Batched**: risky steps pass on the PSU and are collected; one battery session covers them. Every risky step keeps its own tag and image, so a battery failure is bisected by flashing the per-step images in order |

Still operator-owned: any change to the S1 acceptance bands.
