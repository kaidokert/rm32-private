# Graybeard review — firmware50 at the open-loop stage (2026-09-20)

Read: `Cargo.toml`, `lib.rs`, `bemf.rs`, `commutation.rs`, `protection.rs`
(first 270 lines), `bin/shell-pwm.rs` (header + skeleton), `scripts/isr_audit.py`,
`FIRMWARE_CRATE_REBUILD_TASK.md`. Reference lines re-read in `AM32/Src/main.c`
today.

## Verdict

Right shape. Typed policy instead of a feature matrix; protections in the
composition, not optional; every hot-path computation division-free and total
(no panic path); host tests that pin the *reference's* arithmetic (the
`mapped_filter` exhaustive check is exactly how to port `map()`); PAC escapes
each justified in place; the header that **corrects its own inherited scar**
about the HAL PWM macro instead of repeating it. The isr_audit that fails
closed on unreviewed loops and unresolved indirect calls is the tool the old
crate never had. Keep all of that.

Two things will bite on the bench, and both are cheaper to fix today.

## Bug 1 — `mode_for` is inverted relative to the reference

`bemf.rs:116-122`:

```rust
if average_interval < POLLING_CHANGEOVER { Mode::Polling } else { Mode::Interrupt }
```

AM32 (`main.c:2062-2064`): `if (commutation_interval < polling_mode_changeover)
{ old_routine = 0; enableCompInterrupts(); }` — a **small** interval (fast
rotor) means **interrupt** mode. And `:908-910`: `if (average_interval >
polling_mode_changeover + 500) old_routine = 1;` — a **large** interval (slow
rotor) means **polling**. firmware50 has it backwards, and the test at
`bemf.rs:309-315` enshrines the inversion.

The test's comment shows where the model went wrong: *"at 10% the interval is
far above the changeover — `ZeroCross::new(20_000)`"*. It isn't. `ci` is the
**commutation** interval in half-µs. The qualified 10% point is ~400 eHz →
2400 commutations/s → **~830 half-µs**, not 20 000. (20 000 half-µs is 10 ms
per commutation ≈ 17 eHz — below even the 50 eHz startup catch, which is
~3 300.) So with the inverted comparison the 10% rung lands in `Polling`, and
the whole persistence/`offer` path you've written and tested never runs.

Fix: flip the comparison, rewrite the test to assert `mode_for(830) ==
Interrupt` and `mode_for(3_333) == Polling`, and re-seed the other tests'
`ZeroCross::new(...)` values from real numbers (`830` at 10%, `160` at 50%).
The blanking/accept tests are unit-agnostic and will still pass; the
`converges` test should converge onto 830 from 3 333, which is the actual
startup→10% transition.

While there: `POLLING_CHANGEOVER = 2_000` is AM32's default, and on **this
rig** stock AM32 with that default *did not start* (E527, zc 2); halving it to
1 000 via `bi_direction` did (E528–530). The qualified binz image never relied
on AM32's polling startup at all — it used the forced 50→200 eHz staircase and
handed off at ~200 eHz. Reproduce that, and treat the changeover constant as
something to *verify* on this motor, not inherit.

## Bug 2 — `COM_TIMEOUT_MIN = 200` will clamp every commutation late from ~12%

`commutation.rs:229` says the one-shot "accepts only" 200..8000 half-µs. Look
at what the schedule actually needs (`wait = ci/2 − ci·level/64`):

| duty | ci (half-µs) | wait @ level 16 | wait @ level 26 |
|---|---|---|---|
| 10% | ~830 | 208 | 78 |
| 25% | ~330 | 83 | 31 |
| 50% | ~160 | 40 | 15 |

Everything below 200 gets clamped **up** to 200 → the commutation fires
100 µs after the ZC regardless of speed. At 25% that's ~30° late; at 50% the
qualified image ran at `wait = 15`. This clamp alone caps the crate at roughly
the 10% rung and turns every rung above it into late-commutation braking — the
exact mechanism that cooked motor #1. I suspect the 200 was transcribed from
the old image's *event-minimum guard* (238 → 100 µs, itself a ratchet you
removed), not from any property of TIM16, which will one-shot any count ≥ 1.

The real lower bound on `wait` is physical, not numeric: the COM timer must
fire *after* the COMP ISR has finished arming it, so `wait_min ≈ R_COMP` — the
comparator ISR's worst-case response time, a few µs = single-digit half-µs
ticks. Set `COM_TIMEOUT_MIN` from a measured `R_COMP` (see below) with a
comment saying so, keep the max, and add a test that `clamp_com_timeout(15)
== 15`.

## When the ISRs land — build them WCET-shaped from the first line

You already have the audit walking the roots. Three additions make it a WCET
tool, not just a helper-scan:

1. **Longest-path cycle count.** Per basic block: 1 cycle per ALU op, 2 per
   `ldr`/`str`, 1+N per `ldm`/`stm`/`push`/`pop`, 2 per taken branch, 3 per
   `bl`; unroll loops by their allowlisted bound; report the longest path per
   root plus 25 cycles entry/exit. On an M0+ that number *is* the timing
   (no cache, no speculation); add ~2 per taken-branch target for the 2 flash
   wait states if you want pessimism.
2. **Response time, not body.** COMP and COM are peers, so
   `R_COMP = 25 + W_COMP + W_COM + W_guard + W_DMA`. That is the number
   `COM_TIMEOUT_MIN` derives from, and `R_COMP < wait(ci_max)` is the design
   inequality for every rung above 50%.
3. **One SysTick high-water mark** (read at entry/exit, max-store; ~10 cycles)
   in the production image — measured max must sit under the static bound, or
   the model is missing a path or a blocker. The GPIO-pulse-on-a-spare-pin
   check on the operator's scope is the zero-cost version for a one-off.

Shape rules that fall out of that: timestamp **first**; per-step tables (you
already have `SECTORS`; add expected polarity, INMSEL word, and the next
step's gate plan so the COMP/COM bodies are straight-line loads); no
`interrupt::free` and no `bl` on the COMP path — `ZeroCross::offer` takes a
closure and a policy trait, which is fine *only* if the audit shows it fully
monomorphized and inlined into the root; **constant-time persistence** —
the worst case is the full pass anyway, so make it the only case and the loop
has no data-dependent exit. And keep the existing rule: nothing from the
foreground shell ever writes to UART inside a root.

## Protections — one design choice to make now, not transcribe

`FastBusSag` (`protection.rs:164-215`) is a faithful copy of the old image's
guard, resting-bus reference and all. That reference is what turned the
"5% sharp dip" rule into a 1–2% notch rule once the loaded bus had drooped
3–4% (`GRAYBEARD_SAG_REFERENCE.md`; the 8-scan rings show the pre-trip bus at
96–97% of resting in every 45–50% trip). This crate is the place to build it
right: **two references, same DMA path** — *sharp* = three scans below 95% of
a ~200 ms IIR-filtered bus (one shift-add per scan), *slow* = the absolute
floor you already have (`BUS_FLOOR_NUM`). Same threshold, same streak, same
latch, same reason code; only the reference moves. Keep the resting baseline
in the post-run report so droop and dip are both visible.

`PhaseCodePolicy::RetainRails` is the right call given the false-stop
history — but count rails per block and report them. A block with dozens of
rails is information the average erases.

## ADC — two notes for the climb

- 9 901 Hz against a 10 kHz carrier to sweep the sample phase is a good idea
  for **averages**; be aware a 5-channel scan at 79.5 cycles is ~25 µs, i.e.
  2.5 carrier periods at 10 kHz and ~1.2 at 48 kHz, so individual samples
  within one scan sit at different PWM phases. Fine for the block average;
  it's why single-sample checks were never trustworthy. When the BEMF carrier
  moves to 48 kHz, PWM-synchronous mid-ON sampling (TIM1 TRGO + RCR
  decimation, shorter SMP) becomes the alternative worth an A/B.
- `ADC_SPIN_LIMIT = 20_000` iterations is bounded but not *budgeted*: at
  ~5 cycles/iteration that's ~1.5 ms, inside a control tick whose guard
  allowance is 200 µs. Express the limit in TIM17 microseconds so the
  `AdcTimeout` protection and the tick budget speak the same unit.

## Order I'd suggest

1. Fix `mode_for` + re-seed the tests from real intervals (today).
2. Replace `COM_TIMEOUT_MIN` with a derived constant and a `15 == 15` test.
3. Build the two-reference sag guard while `protection.rs` is fresh.
4. Land COMP/COM roots with the cycle-count audit and the SysTick high-water
   from the first flash; get `R_COMP` as a number before the first BEMF handoff.
5. Then the 10/25/50 reproduction, in that order, with the staircase startup.

Everything above is a small edit. The architecture doesn't need to change —
that's the compliment.

— the minz graybeard
