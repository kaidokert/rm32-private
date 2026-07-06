# FALCON envelope hardening — plan & status

Plan drafted 2026-07-05 after v3.4 kept desyncing under sustained
acceleration ("the killer"). Updated 2026-07-06 after the killer was
caught. **Headline: the killer was not an envelope limit at all** —
see item 1 — so several items below changed from "required" to
"optional robustness/performance work".

Current state: closed loop tracks the throttle in both directions
(amp 10→12→10 ⇒ 254→312→249 Hz, ~37 k commutations / 25 s, zero
desyncs, 33–66 mA) on the uncapped board. Commit `6a0fbb0`.

---

## 1. Know the killer — black box + CL stats ✅ DONE (killer caught)

- [x] **Event ring in firmware**: 64 events (REF/BLD/DRK commutation
  classes, ACC accepted-ZC, NOZ no-qZC window, DIS candidate discard,
  ENG engage, DSY desync), frozen at the desync instant, dumped
  decoded (`bb +…us TYP sN d=…`) after the desync message. Lives in
  motor_tester2 permanently.
- [ ] `i`-line refined-vs-blind commutation counters per second —
  not added; the bb dump made them unnecessary for the hunt. Cheap
  to add if wanted for live health monitoring.
- [ ] Re-run `falcon_stats.py` with a 1-confirmation rule (data
  already on disk) — still outstanding; would halve acceptance
  latency if 1-confirm is also ~0 % premature.

**FINDING — the actual killer**: the TIM7 desync watchdog read
`ticks_10us()` BEFORE `LAST_COMM`. A commutation ISR (higher
priority) preempting between the two reads stores a newer
`LAST_COMM`; `wrapping_sub` underflows to ~4×10⁹ µs of "silence" and
a **healthy** lock is killed (bb tail: ACC → REF at 540 µs/309 Hz →
DSY 20 µs later with saturated garbage). Trip probability ∝
commutation rate, which is why it masqueraded as an acceleration
ceiling across v3.1–v3.4. Fixed: load the cross-ISR reference before
`now` + top-bit clamp. General rule recorded in CLAUDE.md.

## 2. Self-healing chain (re-kick before kill) ⬜ OBSOLETE-ish

The "mystery LPTIM stall" this guarded against WAS the watchdog race
— there is no known real stall mode left. Optional cheap insurance
if a genuine chain stall ever shows in a bb dump (signature would be:
long gap with no REF/BLD/DRK events before DSY, since ≪ saturated).

## 3. Kill the systematic acceleration lag ⬜ OPTIONAL (perf)

Not the binding constraint at bench ramp rates — post-fix, amp steps
of +2 tracked cleanly with the trailing ¾-smoothed estimator and
2-of-6 dead-reckoned windows. Still real physics for *faster* ramps:

- [ ] Anchor dead-reckoned C windows to the last real ZC
  (`next = zc + T/2 + n·T`) instead of the previous commutation time.
- [ ] Slope-aware estimator (track dT, or α=½ while intervals shrink
  monotonically) to remove the ~3-window group delay.

Do these when the envelope sweep (item 6) shows where ramps break.

## 4. Relax the gate to ~15–20 % ⬜ OPTIONAL (perf)

Still at 30 % of measured interval. The ADC-sign confirmation's 0 %
premature rate means the gate's only job is skipping commutation
flyback; relaxing buys phase-lead budget during catch-up. Pair with
item 6 to measure the benefit.

## 5. Throttle slew limiting ⬜ TODO (production hygiene)

Rate-limit amp changes (~1 %/50 ms). Not needed to survive bench key
presses post-fix, but correct for any real throttle source and it
bounds the tracking bandwidth the loop must guarantee.

## 6. Measure the envelope ⬜ TODO — the natural next step

Scripted CL ramp sweep: engage, ramp amp at increasing rates, record
where lock breaks → CONDOR-style closed-loop envelope map (max
trackable acceleration vs speed). **This is HEDGEHOG's new headline
metric**: "how much wider do the caps make the envelope" beats edge
counts as the A/B.

---

## Suggested order from here

1. Item 6 (envelope sweep) — establishes the baseline that decides
   whether items 3/4 are worth doing at all.
2. Item 5 (slew limit) — ten lines, do alongside.
3. Items 3/4 only where the sweep shows breakage.
4. HEDGEHOG: same sweep on the capped board, overlay.
