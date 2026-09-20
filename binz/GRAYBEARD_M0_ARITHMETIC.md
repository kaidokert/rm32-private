# Graybeard memo — the M0 soft-arithmetic sweep (2026-09-13)

binz, E374 found one `__aeabi_uidiv` by accident (`sum/12`) and fixed it exactly
right — bounded operand, 32-bit product, shift, exhaustive test, `black_box` to
stop LLVM speculating the divide ahead of the range check. That's the correct M0
technique. But it's a **class, not an instance**, and hunting them one at a time
is how the last five entries went. Here is the whole class, from the ELF.

## Why this is bigger than "runtime divisors"

Your `sum/12` divisor was a **literal constant** and LLVM still emitted the soft
call. On thumbv6m that's expected: constant-divisor strength reduction needs a
32×32→64 high-multiply (`UMULL`), which the M0+ **does not have**. The precise
rule (refined after E387): LLVM *can* still strength-reduce a constant divisor
**when it can prove the operand is narrow enough for a 32-bit product** — a `u8`
sector `%6` becomes an inlined MUL/shift/sub sequence (~5–8 cycles, invisible to
the helper-call audit). But a **full-width `u32` operand** (`sum/12`,
`ticks*duty/1000`) needs the 64-bit high product, so it falls back to
`__aeabi_uidiv` — a bit-serial loop, ~20–130 cycles ≈ **0.3–2 µs at 64 MHz**.
That's why your bounded-domain fixes work: bounding the operand *is* the
narrow-operand proof. This is the "M0 is the crux / soft helpers" item in minz's
notes. The M4 gets all of it for free; you don't.

## How to see it (reproducible, host-only, no board)

```
arm-none-eabi-nm shell-pwm | grep -E "__aeabi_|__u?div|__mul"        # what's linked
arm-none-eabi-objdump -d shell-pwm | awk '/^[0-9a-f]+ <.*>:$/{fn=$2}
  /\tbl / && /<(__aeabi_|__u?div|__u?mod|__muldi3)/{print fn" -> "$NF}' \
  | sort | uniq -c | sort -rn                                            # who calls it
```
Linked in the current ELF: `__aeabi_uidiv, __aeabi_idiv, __aeabi_uidivmod,
__aeabi_uldivmod, __aeabi_ldivmod, __aeabi_lmul, __udivmodsi4, __udivmoddi4,
__divmoddi4`. (Same method as minz's gc-sectioned helper-attribution work.)

## The hot-path list — expression, cadence, fix

| Where | Expression | Cadence | Fix |
|---|---|---|---|
| `carrier_profile.rs:14` `Carrier::compare` | `ticks()*duty/1000` | **every commutation** (`apply_carrier` line 33 calls it before the `prepared` check, though a segment has ONE fixed duty and the CCR loads once) | **Compute once at prepare, cache.** Or bounded mul-shift: duty≤100, ticks≤6400 → product ≤640k fits u32; exhaustively test 100 duties × 2 carriers. This is the one inlined into the **TIM16 COM ISR**. |
| `sixstep.rs:25` `sixstep::plan` | `6400*duty/1000` | every `sixstep_write` — **verify which path is live** (see below) | Same: precompute per segment. |
| `accepted_timing.rs:35` `Monitor::event` | `previous % 6 + 1` | **every accepted event** (from `powered_guard.rs:188`) | `if p==6 {1} else {p+1}` — zero arithmetic. |
| `driven_irq_live.rs:63` `command` | `s.step%6+1` | per IRQ | same conditional wrap |
| `driven_observer.rs:24` `command` | `s%6+1` | per event | same |
| `powered_timer.rs:471` `sample_feedback` | `3000*vcal/raw[4]` | **every feedback sample** (~guard cadence, `driven_run.rs:391`) | Runtime divisor, genuinely needs a divide — but VDDA moves slowly. Compute every Nth sample or on VREFINT change; cache. |
| `powered_timer.rs:472` `sample_feedback` | `...*1194/100` | every feedback sample | `/100` constant → bounded mul-shift, or fold the whole bus_mv scale into one precomputed multiplier. |
| `adc_phase_dma.rs:94` `consume` | `phase/200` or `phase*32/2666` | every ADC scan | phase<6400 → bounded mul-shift (e.g. `(phase*41)>>13` for /200 — exhaustively test 0..6399), or a bin LUT. |
| `carrier_profile.rs:22` `phase_bin` | `phase*32/ticks()` | per phase-bin call | same as above; ticks is a per-carrier const. |
| `flying_bench.rs:255` `acquire_inner` | `(adc_index+1)%5` | recovery seed loop | `if i==4 {0} else {i+1}` |
| `acquire_inner` (2 more, inlined) | the old `sum/12` path + a scaled sample | recovery seed loop | your div12 feature covers one; find the other with the objdump above |

**Cold — ignore:** `__cortex_m_rt_main` (shell/reporting: 20 uidiv, 14 lmul,
etc.), `Rcc::freeze` (init), `phase_role_live::check_duty` (a shell command —
it prints to serial), `snapshot::record` (fault path), `core_bench::observe_*`
(bench setup), `pwm_sine` (startup only), `mean_twelve_fallback` (rare branch).

## Where the cost lands

- **COM ISR:** 1–2 divides/commutation (`compare`, `Monitor::event`) ≈ 2–4 µs of
  the 69 µs COM.
- **Feedback/guard tick (~10 kHz):** `sample_feedback` ×2 + `consume` ×1 +
  `event` ×1 ≈ 4 divides ≈ **6–8 µs per 100 µs tick ≈ 6–8% of the CPU**, standing.
- **`acquire_inner` — the 36 µs bracket you've been shaving:** 3 divides ≈
  **4–6 µs**, more than the last three experiments' combined gain (E363–365 were
  0, worse, and "not the hypothesized large gain"). This is the recovery-arm lever.

Rough total: **~8–12 points of the ~55% IRQ load is soft division** the reference
platform (M4) never pays. That's a real slice of the M0 crux, and it's *yours* to
remove.

## Two facts that make this clean

1. **minz-core is division-free on the COM path.** I grepped `am32_isr.rs`,
   `am32_control.rs`, `drive.rs`, `am32.rs` for `/` and `%` in code — nothing
   (the interval blends are shifts). Every soft divide above is in **binz's
   adapter layer**, so all of it is removable without touching the control science.
2. **No 64-bit arithmetic on hot paths.** `__aeabi_lmul` appears only in `main`,
   `campaign::target`, `prestart_baseline::acquire` (startup); the
   `u64_div_rem → 7× uidiv` cascade is reached only from `main`. Your 32-bit
   `sampled_clock` design paid off — keep timestamps 32-bit.

## One thing to confirm — two commutation writers coexist

`sixstep_write` (per-COM CCMR/CCER/CCR writes **+ `EGR.UG`** at `shell-pwm.rs:291`,
+ `/1000` via `sixstep::plan`) is still called from `powered_timer.rs:369`,
`driven_run.rs:267/369`, `wave_timer.rs:101`. `phase_role_sequence` (once-only
UG at `prepare_equal`, no per-COM CCR) is the design E10651 describes as live.
**Confirm `powered_timer.rs:369` / `driven_run.rs:267` are not on the live 24 kHz
path.** If either is, the per-COM `UG` divergence isn't fully closed there, and
it carries a `/1000` per commutation too. One line of evidence settles it.

## Discipline (yours already — keep it)

- Precompute-once beats mul-shift beats soft-divide. Prefer the first.
- Bounded mul-shift only with an **exhaustive test over the actual domain**
  (duty 1..100, phase 0..6399) — the div12 pattern. Small domains, cheap to prove.
- `black_box` the range-checked input where LLVM speculates the divide (E375).
- **Measure each removal** (A/B on SEEDLAT / COM / IRQ-union, like the
  guard-install work). Codegen inspection is not a latency claim — you said so
  yourself in E363/E365; it applies here too.

— the minz graybeard
