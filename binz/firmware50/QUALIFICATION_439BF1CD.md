# WITHDRAWN — image `439BF1CD` is NOT qualified

> **This document is withdrawn.** The adversarial review
> (`captures/reviews/QUAL-ADVERSARIAL.md`) found four claims below that the
> repo's own captures contradict, and **I verified every one of them myself**:
>
> 1. **Both restart rows are false.** All six "restart at 500/600" runs print
>    `target_duty_tenths=250` — they ran at **25 %**. `Z` takes its duty from
>    `provoke_tenths`, which the `x` key cycles 250→375→475→500→600
>    (`mod.rs:636-648`); my `--rung-duty` set only the *prerequisite* rung and
>    never the duty, and I never sent `x`. **Restart at 50 % and 60 % has never
>    been demonstrated.**
> 2. **"`left == 0` is arithmetically unreachable" is false.**
>    `wait_time(ci,16)` is not `ci/4`: integer truncation gives
>    `wait(40)=wait(41)=10` and `wait(45)=11`. And `spent_max_us = 11` in
>    **18 of 18** live runs that reported it — not the 10 I quoted from a
>    *chain-capture sample* taken at advance 20/22 without `deep-filter`. At
>    `spent = 11`, `left == 0` for every `ci <= 45`, **and `ci_min = 45` was
>    observed**. The real worst-case margin at the excursion floor is **0–1 µs**.
>    I conflated a sampled per-arm distribution with a live whole-run maximum —
>    the exact error E314 was written about.
> 3. **All nine protection injections ran at 15 % duty**, not at a qualified
>    rung, and `prot-I` moved its own threshold 100× (`ma_allow=318` against
>    `ma_allow_ref=31857`). Seven live `Reason` paths have no injection at all,
>    including `LateArm` — the campaign's own hazard.
> 4. **§5 point 5 was wrong.** `states.rs:341` enforces an absolute bus floor
>    on every raw scan, unconditionally, in production. I inherited a stale
>    claim that `BUS_FLOOR_NUM` is Band-policy-only without checking that a
>    separate floor exists.
>
> And a fifth, self-found while checking (1): **the 65 s window I chose to make
> the holds efficient cannot fit a rung-600 restart.** Segment 1 consumes ~34 s,
> leaving ~31 s against a segment-2 `need_ms` of 33 000. It fitted at 90 s. So
> the window must be sized by the *hardest* criterion — the 600 restart — not by
> the holds' convenience, which is also the honest answer to the review's charge
> that 65 s grades on an easier curve.
>
> **What survives:** the rung-600, 575, 525 and 500 hold cohorts are genuine
> (`target_duty_tenths` verified at each), the regression anchors passed, the
> supply anchoring is sound, and `late_arms = 0` / `thin_count = 0` across all
> 21 powered runs is real — but it bounds the hazard to ≤ 0.0287/s at 95 %,
> which *contains* the previous image's 0.0193/s, so it does not yet establish
> reliability.
>
> A corrected campaign runs on a re-windowed image. Nothing below should be
> cited until it is re-issued.

---

# (withdrawn) Qualified image `439BF1CD` — measured envelope and constraints toward 100%

**Deliverable for campaign 11 ("deliver reliable 60% operation efficiently").**
One reproducible ELF, what it was measured to do, and what the evidence says
about going further.

---

## 1. The image

| | |
|---|---|
| file | `captures/elf/439BF1CD.e331-advref-floor5-65s.elf` |
| crc32 | `439BF1CD` |
| sha256 | `FE927FA3AA4C542B92A716A44FCEAC08B10927E3FB8A159AC150E13BA44044CC` |
| build | `cargo build --release --features advance-ref,deep-filter` |
| source | commit `224d8c8` (`am32_sheet`) |

**Reproducibility caveat, measured:** the *loadable* image rebuilds
byte-identical, but `.debug_line` differs between builds, so the **crc32 that
names the file is not reproducible** even though the firmware is. Verify a
rebuild by comparing the loadable sections, not the filename.

Two divergences from the previous production image, both control parameters,
neither a protection:

1. **Advance level flat 16** (`advance-ref`) — *the reference value*:
   `commutation.rs:238` defines `DefaultAdvance = FixedAdvance<16>` and AM32's
   `Src/main.c:656` sets `temp_advance = 16`. The campaign's previous 20/22 was
   a divergence *upward*; this is a return to reference.
2. **Persistence-filter floor 5** (`deep-filter`), against the reference map's
   floor of 3. This *is* a divergence from reference, deliberately.

Plus `BEMF_TOTAL_MS` 90 → 65 s (harness window, not firmware behaviour) and
removal of two dead `accept_wait` stores from the prio-0 COMP root.

Audit state: `ADC_COMP` 742 instructions, hazard classes `div 0, mul 4, irq 9`,
**no support-library call in any of the four ISR roots**, audited 12-read filter
bound intact, 345/347 host tests across four feature configurations, clippy
clean.

---

## 2. What was measured

Every run un-injected unless stated, every protection armed, no foldback
(`ceiling_tenths == duty_tenths`), `ma_allow == ma_allow_ref`.

| requirement | result |
|---|---|
| 3 × ≥30 s holds at 500 | **PASS** 3/3 |
| 3 × ≥30 s holds at 525 | **PASS** 3/3 |
| 3 × ≥30 s holds at 550 | **2/3** — one `FastBusSag` |
| 3 × ≥30 s holds at 575 | **PASS** 3/3 |
| **3 × ≥30 s holds at 600** | **PASS** 3/3 |
| 3/3 restart at 500 | **PASS** — `recovered=1` ×3 |
| 3/3 restart at 600 | **PASS** — `recovered=1` ×3 |
| lower-rung regression | **PASS** — anchors 150 and 375, 1/1 clean each |
| protection coverage | **PASS** — 9/9 |

`hold_ms = 65000 − 5224 − ramp_us(rung)` to the millisecond in every run: 34776
at rung 600, 35776 at 575, and so on. The 5224 ms is the pre-closure phase.

### Rung 600, the target

| hold | reason | `hold_ms` | `late_arms` | `thin` | `ci_min` | `hold_ma` | `worst_ma` |
|---|---|---|---|---|---|---|---|
| h1 | 2 | 34776 | 0 | 0 | 47 | 2322 | 3605 |
| h2b | 2 | 34776 | 0 | 0 | 48 | 2280 | 3246 |
| h3 | 2 | 34776 | 0 | 0 | 48 | 2331 | 3327 |

Restart at 600: `first_reason=8` (the injected `Tracking` fault) → second
segment `reason=2`, `second_hold_ms` 34249/34249/34250, `recovered=1` ×3.

### Protection coverage

| key | protection | expected reason | observed |
|---|---|---|---|
| T | Tracking | 8 | 8 |
| G | TickGap | 3 | 3 |
| F | FeedbackStale | 4 | 4 |
| N | Driver | 7 | 7 |
| U | CompStorm | 13 | 13 |
| H | HandlerOverrun | 14 | 14 |
| V | FastBusSag | 26 | 26 |
| I | AverageCurrent | 25 | 25 |
| W | Watchdog | next boot's reset flag | `RESETCAUSE iwdg=1` |

### The supply, anchored

Operator-confirmed **clamp 3.00 A**; operator-metered **2.15 A** during a
rung-600 hold against the proxy's `hold_ma` 2280 — a **6.0 % over-read**, inside
the 2.7–12.4 % bracket `scripts/metered_current.py` derives from the corpus.

```
true draw at 60 % = 2.15 A = 71.7 % of the clamp
droop 982 per mille (CV threshold 975) -- the rail never folded
```

`hold_ma` is an uncalibrated signed three-shunt residual. **Do not read it as
amps** without that anchor, and note that a `worst_ma` "worst block" is a
10.1 ms mean the PSU's output capacitors supply, so it is **not** a
constant-current criterion. The CV/CC discriminator is `filt_bus/ref_bus`.

---

## 3. The one failure, characterised

`captures/2026-09-25/q550-advref_02.txt` — `reason=26` (`FastBusSag`),
`streak=3 tripped=1`, hold 26 762 ms:

```
late_arms=0   thin_count=0   verdict=ok   zc_permille_of_6x_coast=1013
filt_bus 983 per mille (CV threshold 975)   <- the filtered bus was healthy
bus_min  870 per mille                      <- deepest instantaneous dip of the session
hold_ma  2047   worst_ma 2633               <- LOWER than the passing rung-600 runs
```

Not timing, not tracking, not duty, not the mean rail: a **fast transient**
caught by the guard built for fast transients, **at less current than the
rung-600 runs that passed three times over**. No progressive degradation is
visible — across all 21 powered runs `zero_drift_ma` wanders −114…−278 with no
monotone trend and `filt_bus` holds 980–986 per mille.

**It is recorded, not excised.** `rung_report` counts every attempt on an ELF
permanently, so rung 550 is failed for this image. That rule prevents
retry-until-pass. This bench has a recorded history of deep sag and a melted
connector at ~0.65 Ω, so the open question is the power path, and the standing
recommendation is to measure the sag-versus-current slope before qualifying
through a vbat anomaly.

---

## 4. What actually fixed the failure, and what did not

The campaign's blocker was `Reason::LateArm`: `left = wait − spent` reaching 0,
where `wait = wait_time(ci, level)` and `spent` is the COMP handler's cost at
the arm.

| lever | outcome |
|---|---|
| reduce `spent` | **no** — distributed irreducible work (ISR prologue, EXTI ack, extended-clock read, four `Seam` critical sections). One dead store worth 2 instructions was all there was. |
| `ceil(ci/2)` in `wait_time` | **no** — creates a *new* LateArm path: `spent` is a 1 MHz sample, so +4 instructions raise it a whole µs on ~6 % of arms while the wait does not move on even intervals. Reverted. |
| rate-limit the estimator's descent | **no** — the runaway's maximum step (12.5 %) *is* normal jitter (10.8 %). Not separable. |
| cumulative descent limit | **no** — 1.9× separation on one run, narrowing with duty. |
| widen the blanking gate 32→40/64 | **no** — refuted on the bench: 9 % slower rotor, missed real crossings, `FastBusSag`. |
| persistence floor 3 → 5 | **partly** — 91× on the rung-600 hold, but still 1 clean run in 3. |
| **advance → flat 16 (reference)** | **yes** — `wait = ci/4 ≥ 10` for every `ci ≥ SECTOR_FLOOR_US` against a `spent` capping at 10, so `left == 0` is **arithmetically unreachable**. |

**`late_arms = 0` and `thin_count = 0` in all 21 powered runs on this image**,
including the one that failed on the bus — while `ci_min` still falls to 45–48
exactly as before. The estimator's descent was never the thing worth fighting;
the arm's margin was. And the fix relieved the supply too: `hold_ma` 2774 → 2280
(−18 %), `worst_ma` 3732 → 3246.

---

## 5. Constraints toward 100 %

1. **The advance schedule is now flat at the reference 16, and that is a
   parameter, not a law.** It works because `wait = ci/4` exceeds the measured
   `spent` ceiling of 10 µs at every interval the estimator's clamp permits. At
   higher speeds `ci` shrinks: **`wait = ci/4 ≥ 11` fails below ci 44**, so the
   same guarantee lapses around **ci 40–44 µs (≈ 3800–4200 eHz)**, which is
   also where `SECTOR_FLOOR_US = 40` sits. Past that, either `spent` must come
   down — and §4 says it cannot, locally — or the arm must be restructured.
2. **`spent` is the hard floor on commutation margin**, ~5–6 µs typical and 10
   µs worst over 20 478 measured arms, and it is distributed across the ISR
   prologue, the EXTI acknowledge, the extended-clock read and four critical
   sections. Reducing it needs a structural change to the COMP root, not tuning.
3. **The estimator will descend 30–40 % below the true interval and no
   rate-based test can distinguish that from normal jitter.** Any future design
   must tolerate the descent rather than prevent it — which is what advance 16
   does — or use an independent speed reference (the coast check separates clean
   stops from every latch at 14–45 %, so the information exists outside the
   interval sequence).
4. **There is no thermal channel anywhere in this firmware** (`src/hw/adc.rs`
   has five ADC channels, none temperature) and no thermal protection. 21
   powered runs at up to 72 % of the clamp were done without any thermal
   instrument. This is the largest unguarded risk in the system.
5. **`Reason::PhasePeak` is documented as never raised**, and the only current
   stop folds back on the first over-block and stops only on the second
   (earliest ≈ 20 ms). There is no absolute bus floor in the live image —
   `FastBusSag` is a band-pass blind to both a collapse faster than 0.8 ms and a
   droop slower than 0.2 s.
6. **Two fixture defects found and left for a decision.** `hold_forced != 0`
   can never fire (`run/mod.rs:330` hard-codes it "structurally 0"), so one of
   three qualification gates is dead; and a run the fixture itself declares
   *"cannot be judged"* still permanently fails a rung. Both were recorded
   rather than repaired, deliberately, since repairing a gate mid-qualification
   would be marking one's own homework.

---

## 6. Honest accounting

This image's qualification rests on **21 powered runs in one session**, on one
bench, with one motor. The campaign's own history shows the latch rate varying
**2.9–4.5×** between images with identical control paths, and **22×** between
some images at the same rung — so codegen is a first-order carrier of behaviour
here and a rebuild is not a no-op. `late_arms = 0` across 21 runs is a strong
result against a hazard that previously fired in 1 run of 3, but it is 21 runs.

Not verified by independent review at the time of writing: the final
qualification batch. Two context-free reviews (evidence and adversarial) are the
campaign's standard before a conclusion is accepted, and this document should be
read as the author's claim until those are attached.
