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

## 3+4. Acceleration lag + gate relaxation ❌ ATTEMPTED & REVERTED

Tried together 2026-07-06 (ZC-anchored dead-reckon, asymmetric α=½
on shrinking intervals, gate 30 %→20 %) and produced a **commutation
runaway**: the loop spun its own field to "3333 Hz" (50 µs windows,
scheduler floor) while the rotor did no such thing. Neither guard
fired — commutations never pause (watchdog content) and a field
detached from the rotor draws little current (no OC trip) — so the
sweep printed SURVIVED. Reverted to the proven build (verified
255 Hz lock after revert).

**Mechanism (why it ratchets)**: boosting α only in the shrinking
direction biases the estimator under noise — every junk-short
interval sample is chased at α=½ while recoveries crawl at ¼; the
20 % gate shrinks proportionally with the falling interval, admitting
junk earlier each window; the ZC anchor then propagates the compressed
phase. A one-way-fast filter + self-referencing gate = positive
feedback.

**If ever retried, the safe version needs**:
- a physical plausibility clamp on the interval floor (from vbus/Ke:
  the rotor cannot exceed ~1.2 kHz elec at this voltage);
- symmetric slope handling (track dT properly, not one-way α);
- anchored/relaxed behavior gated on lock quality (e.g. ≥3 accepts in
  the last 6 windows), degrading to the proven plain free-run;
- `cl_ramp_sweep.py` now flags sub-150 µs windows as **RUNAWAY** so
  this failure mode can never read as SURVIVED again.

Given item 6's result (steady lock to ~980 Hz, no ramp-rate limit on
the proven build), the honest cost/benefit says: leave 3/4 unbuilt
unless HEDGEHOG's A/B or a real load shows mid-ramp coverage
actually mattering.

## 5. Throttle slew limiting ⬜ TODO (production hygiene)

Rate-limit amp changes (~1 %/50 ms). Not needed to survive bench key
presses post-fix, but correct for any real throttle source and it
bounds the tracking bandwidth the loop must guarantee.

## 6. Measure the envelope ✅ DONE — no breakage found

`scripts/cl_ramp_sweep.py`: amp 10→16→10 ramps at 500/250/120/60 ms
per step **and instant steps**, MAGPIE streamed throughout, black
box auto-dumps on any break. Results (board 1, no caps, 6.5 V):

- [x] **All five rates SURVIVED** — zero desyncs, zero trips,
  including the instant amp step. No ramp-rate limit exists in the
  tested space.
- [x] Top speed reached **~980 Hz electrical (~8,400 RPM mech)** at
  the 60 ms ramp — **above the rinz board's ~700 Hz envelope**, on
  the uncapped board.
- [x] Speed-resolved coverage (qzc%/pred% bucketed by f_e): **every
  steady plateau reads ~100 %** (600-699 Hz hold: 100 %; 800-999 Hz:
  100 %). Coverage dips to ~20-30 % **during accelerating passes**
  (esp. 400-500 Hz transits) — the free-run + dead-reckon bridge
  carries those windows. Sector asymmetry at speed: sec 4 stays 93 %
  while sec 1/2 collapse mid-ramp (worth a WAXWING burst at ~700 Hz
  if item 3 is ever built).

**Envelope statement**: steady-state lock is clean to ≥980 Hz;
transient coverage thins mid-ramp but never broke. Items 3/4 now
have a measurable target (raise mid-ramp coverage from ~20 % —
robustness margin, not a functional need at bench load).

## 7. Post-stall hardening (2026-07-06, after the bench stall) ✅

Two real defects found when the rotor stalled during an unguarded
lock-map sweep and the firmware kept driving (operator caught it by
eye — the rinz lesson again):

- [x] **ZC-starvation watchdog**: free-run scheduling made the
  commutation-recency watchdog meaningless (a zombie field never
  stops commutating and draws little current). New: no ACCEPTED qZC
  for 12 intervals while CL → kill, `!! CL ZC-STARVED`, bb `STV`
  event. The loop's blindness is machine-detectable even when the
  rotor's state isn't.
- [x] **Harmonic lock killed**: the estimator's sanity band only
  bounded interval GROWTH, so half-period junk walked it down and the
  loop could lock at 2× rotor frequency ("539 Hz" qzc 28 % vs true
  257 Hz qzc 100 % — also explains suspicious ramp-sweep peaks).
  Fixed with a symmetric rate bound: new ∈ [0.6, 1.8]×old.
- [x] `cl_lock_map.py`: full session log (no buffer resets eating
  kill messages), `cl: ACTIVE` check before each point, qzc<50 % =
  BLIND → abort, breakage stops the sweep (no blind re-engagement).

**Lock map result** (`captures/lockmap_map.png`): amp 9→16 ⇒
230→416 Hz, ~27 Hz/%, monotone; **qzc 100 % at every point, all
sectors** (62.5 k windows); current 45→99 mA; window jitter 9-13 %
(speed wander, not lock loss).

**Extended map to amp 20** (`captures/lockmap20_map.png`): clean
100 % locks through **amp 18 / 467 Hz**; at amp 19 the loop went
blind (every window NOZ — raw edges present, none confirming) and
the **ZC-starvation watchdog made its first live catch**: 12 blind
intervals → controlled kill, ~6 ms of blindness total, script
stopped itself. The true envelope edge is the 2-wrap confirmation
latency no longer fitting the ~350 µs windows near 480 Hz. Known
lever if more speed is wanted: the 1-confirm rule (offline
falcon_stats replay on existing captures would justify it — halves
acceptance latency). Also learned: engage ascending (dropping to
amp 9 immediately after engage lands in a degraded regime — the
BLIND guard caught that too; sweep amp 9 on the way down).

## 8. Voltage + confirm-depth experiments (2026-07-06, 7.5 V bench)

The ~480 Hz ceiling is **speed-domain, not amplitude**: at 7.5 V the
same wall appears at lower throttle (even amp 10's engage spin-up
crossed it and ZC-starved — guard caught it cleanly). The 1-confirm
rule was then justified offline (`falcon_stats --confirms 1` on the
probe captures: CL conditions 100 % accept / 1 % premature /
+0.1±0.7 frames) and shipped (`CL_CONFIRMS=1`); at 7.5 V it gives
clean 100 % locks amp 10-15 = 296-442 Hz
(`captures/lockmap75_map.png`) — but the wall stayed at ~475 Hz, so
confirmation depth wasn't the binding constraint either.

Failure anatomy at the wall (bb): one over-long accept walks the
estimator up (symmetric bound allows ×1.8), the gate — proportional
to the ESTIMATE — then rejects the rotor's now-"early" real ZCs
forever, and the starvation guard kills. Prime physical suspect for
why timing goes bad near 480 Hz: **commutation advance is still 0°**
(AM32 defaults ~16°; phase lag grows with speed). A quick advance=20°
try failed at ENGAGE because ADVANCE_DEG also shifts the open-loop
sector mapping — the proper experiment is engage at 0° then step
advance under CL, a session of its own.

## 9. Advance sweep under lock (2026-07-06) — advance is NOT the lever at no-load

`cl_adv_sweep.py` + `plot_adv_map.py` (`captures/advmap_map.png`):
engage at 0° (open loop consumes ADVANCE_DEG — engaging at 20°
breaks spin-up), then step advance through 0/20/40/−40/−20 under
lock at amp 11-15, 7.5 V.

- Healthy rows (amp 12/13/15): **+20°/+40° changed nothing** — speed
  flat (356/390/445 Hz), current flat (71/81/102 mA), lock coverage
  mildly down (100 → 84-95 %). At no-load the phase current is tiny,
  so phase lag — the thing advance compensates — barely exists.
  Advance becomes interesting under LOAD (prop / HEDGEHOG), not on a
  bare bell.
- **Negative advance is destructive**: retard collapses the rotor to
  a ~5 Hz crawl at 250-440 mA (and the loop *tracked the crawl* at
  qzc 70-84 % — grimly impressive).
- The 480 Hz wall therefore stays attributed to loop/estimator
  dynamics near the ceiling, not to missing advance.
- Recurring theme in the raw log: the **engagement lottery** — amp 11
  and 14 rows landed in degraded regimes (qzc 17-39 %) after
  re-engages. Post-engage quality validation with auto-retry (check
  qzc over the first ~50 windows, re-engage if <90 %) is now the top
  remaining robustness item.

## 10. Late-session regression + fixes (2026-07-06 evening) — bench needs rest

- [x] **Unconditional 1-confirm was a regression** and produced engage
  runaways (144 µs "1157 Hz" fields): 1-confirm is only clean in
  LOCKED conditions; during the open-loop→CL handoff it is 52-69 %
  premature and seeds the estimator with junk. Fixed: **conditional
  depth** — 2 confirms until `CL_ACTIVE`, 1 after.
- [x] **Firmware runaway floor**: interval < 160 µs (~1 kHz elec — no
  plausible rotor here) → kill. Runaway self-sustains on junk accepts
  so the starvation guard alone can't catch it.
- [x] `cl_lock_map.py` engage now VALIDATES lock quality (1.2 s
  sample, qzc ≥ 90 %, ≤4 retries) — the engagement-lottery TODO,
  mechanized.
- ~~Unresolved: progressive engage failure~~ **ROOT-CAUSED & FIXED**
  (not thermal — motor was stone cold and it persisted): a poisoned
  `OWL_INTERVAL` (144 µs left by a runaway) was **unrecoverable by
  design** — the runaway floor killed every engage instantly while
  the symmetric bound rejected every honest open-loop sample
  (1667 µs ≫ 1.8×144). Fix: the `y` arm handler resets the estimator
  (interval=0, chain broken); engagement already waits for a fresh
  estimate, so it re-seeds from open-loop qZCs in a few windows.
  Guards that each do their job can still deadlock as a SYSTEM —
  reset-on-arm is the escape hatch.

**Prop-loaded high-amp results (7.5 V, after the fix)**:
- adv 0: clean 100 % locks amp 11-15 = 328-442 Hz; wall at amp 16 /
  474 Hz **with qzc 98 %** — the wall is loop timing/dynamics, NOT
  sensing (the comparator+ADC pipeline is essentially perfect at the
  wall).
- adv +20 under lock at amp 10: ZC-starves within 12 intervals —
  advance shifts the ZC to ~83 % of the (shortened) window and the
  current gate/confirm geometry stops accepting; the advance path
  needs its own geometry pass before the prop-loaded advance question
  can be answered. TODO with fresh eyes.

## 11. Re-acquisition + 48 kHz campaign (2026-07-07)

**Re-acquisition mode ✅ (the envelope lever that landed)**: after 2
consecutive ZC-less A/B windows, gate drops 30 %→8 %, confirm depth
returns to 2, the qZC chain is broken (recovery measured from two
FRESH strict ZCs), interval re-seeds bounded [0.5, 2.0]×old. Normal
accepts tightened to ±25 %/window (blocks the ~1.5× aliased accept
that seeds the lockout spiral). bb event `RAQ`. **Wall moved: amp
16/17 (475/500 Hz) now run at qzc 100 %; break relocated to ~amp
18/522 Hz.** Two hard-won sub-lessons: the direct re-seed MUST be
bounded (unbounded version re-seeded an aliased 162 µs and tripped
the runaway floor), and the normal gate is sensitive (a transient
30 %→20 % change let early noise edges reach the 1-confirm fast path
and destabilized amp 12).

**48 kHz PWM ❌ implemented, characterized, reverted**: the theory
(halve the 42 µs confirm quantum) was sound; practice taught:
1. **ADC trigger floor ≈ 1.0 µs** — dead-time (562 ns) + gate-driver
   + FET turn-on; a 0.6 µs trigger sampled ch9 while the phase node
   was still LOW (WAXWING: channel A flatlined 0 —
   `captures/wax48_083523.png`; textbook again at 88 ticks —
   `wax48b_191853.png`).
2. **The comp blank must scale with the carrier** — 20 µs covers an
   entire 48 kHz period; every edge suppressed, zero candidates,
   loop self-blinded (bb: 170-300 raw edges/window, all NOZ).
3. **Noise density doubles and wins** — after both fixes CL engaged
   and validated (qzc 94-100 %) but the envelope DROPPED to ~260 Hz
   vs 500 Hz at 24 kHz: ~1.8 noise edges/PWM cycle churns candidates
   faster than confirms accumulate; blank tuning (8/12 µs) didn't
   converge. A real 48 kHz campaign needs a candidate-hold policy
   (don't reset the candidate on churn) and likely HEDGEHOG's caps.

Reverted to 24 kHz; ladder re-verified 100 % through amp 15-17.

## 12. Second 48 kHz push (2026-07-08 pre-dawn) — WIP branch `wip_48khz_campaign`

Additional findings, all preserved on the WIP branch:
- **24.5-cycle sampling minimum** with the 3.3 kΩ sources: 12.5
  cycles broke sectors 2/4 specifically (S/H inter-channel drag —
  owl_report per-sector table was the diagnostic); 24.5 restored
  sector 4 and collapsed the prediction bias +44 % → −0.3 %.
- **Sector 2 anomaly, unresolved**: qzc 0 % at 48 kHz with healthy
  raw/valid edge counts AND a healthy analog arc, robust to swapping
  the confirm rule to VBUS_EST. `wax48s2_074609.png` shows phase A
  *rising* through neutral in sector 2 where the convention expects
  falling — the PHASE_MAPPING deep-dive needs redoing with a scope.
- Wrap-armed candidates (pure ADC-sign arming, no comp edge) built
  and regime-gated (CL-only + fast-window <700 µs) — the open-loop
  sign rule is 29-41 % premature, so wrap-arm outside a fast lock
  poisons the estimator.
- Commanded-rate estimator seed at arm + ARMED-freeze — has a
  residual units/order bug (seeds ~278 µs against a 1667 µs rotor);
  this bug also breaks the consolidated 24 kHz build on the WIP
  branch. Root not found — fresh-eyes item.
- **Bench drift is real and repeatable**: at session end even the
  bit-identical known-good build (amp 17 the previous evening) broke
  at amp 11-12. Same pattern as the previous night (recovered after
  rest). Long sessions poison ladder comparisons — calibrate against
  a known-good control run before trusting any A/B late in a session.

## 13. 48 kHz ✅ WORKING — the minimal recipe (2026-07-08)

After the bench recovered (control ladder 100 % to amp 17), round 4
succeeded by combining ALL the hard-won constraints with ZERO logic
changes — every logic "enabler" from rounds 1-3 had been convicted
by A/B and reverted; the winning build is the known-good loop
untouched:

- PWM 48 kHz (ARR 1665);
- **phase channels keep full 47.5-cycle sampling** (required — the
  sector-2 razor-margin rule), current channel ch8 drops to 12.5
  cycles (INA180 is a low-impedance op-amp output) — the saved
  437 ns brings the sequence end to ~3.06 µs;
- trigger stays 0x64 = 1.25 µs (the proven point);
- **engage at amp 15** (the ON window that fits the sequence);
- comp blank 8 µs (scaled for the carrier).

**Result (`captures/pwm48final_map.png`, 7.5 V, prop): amp 15-20 all
qzc 100 % — 405 → 555 Hz, 52 800 windows, ZERO breaks.** The old
~480 Hz confirmation wall is GONE (21 µs confirm quantum), amp 19/20
— which starved at 24 kHz — run clean, and no new wall was found up
to the amp cap. qZC position σ ~9 % of window (vs 11-13 % at 24 kHz).

Trade-off: the CL floor is amp 15 (~405 Hz) — below that the ADC
sequence doesn't fit the ON window, so low-speed closed loop needs
the 24 kHz build. Carrier choice is one constant + engage amp +
blank; a runtime carrier switch (engage low at 24 kHz, shift to
48 kHz at speed) is the obvious future unification.

## 14. SWIFT — AM32-style edge path lands the throttle range (2026-07-09)

Question that drove it: AM32 runs this exact hardware to 100 %
throttle — why can't we? Answer: our ADC-sign deferred confirmation
quantizes ZC acceptance to the PWM wrap (21 µs @ 48 kHz) + 1-2
confirm wraps; that fixed per-window latency collapses the schedule
margin as windows shrink → the ~620-650 Hz ceiling. AM32 timestamps
the comparator edge and schedules immediately — nothing in its
timing scales with speed. (Also: the amp cap was raised 25 → 50
along the way; the ~640 Hz breaks proved speed-referenced, not
throttle-referenced, and the PSU was exonerated twice — vbat flat
under load once per-point voltage logging was added to the lock
maps.)

**SWIFT** (`M` key; `cl_lock_map.py --fast`): when LOCKED, a comp
edge that passes the gate + 5-read persistence is accepted
IMMEDIATELY in the COMP ISR — µs-precision, zero wrap latency,
AM32's architecture. Engage + low speed keep the adc-confirm path
unchanged (they own the noise-limited small-BEMF regime — the part
AM32 is famously rough at and we are not). All guards apply to both
paths. Same-session A/B (drifted-bench state):

| path | result |
|---|---|
| adc-confirm (control) | broke amp 22 / 622 Hz |
| SWIFT | **qzc 100 % through amp 26 / 739 Hz**, first break amp 28 @ 769 Hz |

+117 Hz over control — and qZC position jitter FALLS with speed
under SWIFT (9.6 → 8.4 % of window), inverse of the adc path: the
signature of latency-free detection. `captures/swift1_map.png`.

Open: where SWIFT's own ceiling is (769 Hz break was on a drifted
bench at 97 % coverage — rested-bench ladder pending), and a
speed-scheduled auto-switch (adc-confirm below ~450 Hz, SWIFT
above) instead of the manual key.

## 15. Speed-adaptive blanking — the amp-28 wall falls (2026-07-09)

Correction first (user's, and right): AM32 absolutely has stall /
desync protection (zero_crosses resets, stuck-rotor handling) — the
§14 "no kill switch" framing was unfair. What it does differently is
*recover in place* rather than kill-and-report; our kill-and-dump is
a bench-forensics choice.

The cribbed mechanism (`AM32/Src/main.c:2112-2119`): on L431, AM32
has **no time-since-PWM-edge blank at all** — its noise defense is
persistence depth scaled with speed (`filter_level =
map(average_interval, …)`, 12 reads slow → 3 fast; hardware COMP
blanking is a G071-only feature). Our fixed 8 µs blank after every
PWM CC edge (~2 per 20.8 µs cycle at 48 kHz) left us blind ~75 % of
every window — the real cause of the SWIFT amp-28 break.

**Fix (COMP ISR, both ZC paths)**: effective blank =
`min(user_blank, interval/75)` µs — shrinks with speed (3 µs at
740 Hz, 2 µs at 1.1 kHz); once it falls below 5 µs the persistence
check deepens 5 → 12 reads (AM32's own value for our interval
range). At low speed the arithmetic yields exactly the proven
8 µs + 5-read combo — nothing changed below ~600 µs intervals.

**Result (`captures/swiftblank_map.png`)**: the ladder ran off the
end of the amp list — **first no-break ladder of the project**. amp
18-32 all qzc 100 %: 512 → **864 Hz** (≈7,400 RPM); amp 34 still
locked at 99 %/811 Hz with the speed curve bending (real V/f
saturation at ~2.5 V applied, and vbat starting to dip 7.44→7.35 —
lead IR at 570 mA). 117 k windows. The remaining envelope above
~850 Hz is volts and mechanics, not firmware.

## 16. Phase 1 + the amp-cap saga — 970 Hz @ 100 % (2026-07-09/10)

The "reach full throttle" chase, compressed: AMP_MAX raised 25 → 50
(each cap's BEMF equilibrium had been masquerading as a "voltage
wall"); the PSU exonerated twice (per-point vbat now logged in the
lock maps + `<tag>_meta.csv`, plotted red on the current panel);
prop ω³ current identified; then the amp-34 speed DROOP correctly
re-read as **late-commutation braking, not V/f saturation** (a
plateau flattens, a droop means mistimed) — static adv 8° = +108 Hz
at amp 34.

**Phase 1 shipped** (all A/B-validated in one ladder):
- auto-advance ramp: 0° below ~280 Hz → 12° cap by ~1.2 kHz,
  manual t/T overrides; beat static 8° at every rung;
- µs-resolution gate (SECTOR_START_US / SECTOR_GATE_US);
- telemetry decimation (every 5th window above ~925 Hz, coprime
  with the 6 sectors).

**Record: amp 35 = 970 Hz (~8,300 RPM) at qzc 100 %**
(`captures/phase1_map.png`); marginal edge amp 36.

Meta-lesson worth its own line: **label conclusions with the regime
they were measured in.** "Advance is not a lever" (§9) was true at
400 Hz / 90 mA and cost a day when treated as global; same pattern
as "1-confirm is safe" (true only under lock) and "caps required"
(true only pre-software-filtering).

---

## Suggested order from here

1. ~~Item 6 (envelope sweep)~~ — done; baseline banked in
   `captures/ramp_r*.bin`.
2. Item 5 (slew limit) — still worthwhile production hygiene.
3. Items 3/4 — optional; target = mid-ramp qzc coverage, verified by
   re-running the same sweep.
4. HEDGEHOG: identical sweep on the capped board, overlay the
   speed-resolved coverage curves — the headline A/B.
