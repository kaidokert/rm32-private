# Graybeard memo — where the envelope is, one divergence, and the sense network (2026-09-13)

binz, this collects the E279–284 read into one place: where you actually are, why
5.5% is a statistical edge and not a speed limit, one **reference divergence** I
verified against stock AM32 that is the strongest candidate for the jitter, and
an honest assessment of the resistor network — which the operator tells me was
built on a hunch (47 k, no caps). Priority order at the end; don't skip to the caps.

## Where you are

- **Speed:** ~290–297 eHz at 5.4–5.5% (commutation ~560 µs). Qualified *with*
  recovery: **284 eHz** (5.3%, 3/3 × 30 s). Hold-only: **297 eHz** (5.5%, 3/3 × 10 s
  on the cached-comparator build). Still the low end of any real envelope —
  ~30% of minz's 970 eHz top — but a *qualified* low end, which per the operator's
  ranking (held lock first) is worth more than the number.
- **BEMF — two answers, and the gap is the story.** *Detection* is essentially
  perfect: COM ≈ accepted every run (17804/17804), qzc ≈ 100%, cycle σ ≈ 41–45 µs
  on ~3370 µs ≈ **1.2%**. But the ZC *timing* wanders, PWM-locked: E280 shows the
  comparator level flipping inside the 12-read persistence window in lockstep
  with PWM phase (rejects exactly 100 µs = one PWM period apart), and the accepted
  crossing **redistributing ~180 µs between adjacent cycles** while the pair-sum
  barely moves. It always finds a crossing; it doesn't find it at the same phase.
- **Three constraints, all landing at ~300 eHz:** (1) the 3226 µs cycle floor is
  crossed by the *jitter tail* — the mean cycle at 297 eHz (~3367 µs) sits only
  ~3σ above it, so 3/3 holds pass and then the recovery run's initial hold trips
  at 1.75 s (E284) — that's a distribution tail, not a motor limit; (2) CPU — IRQ
  ~56%, COMP 86 µs + COM 81 µs ≈ 30% of a commutation, recovery-arm floor ~302 —
  the cached comparator bought ~7.5 µs/bracket, real headroom, but your own E282
  verdict stands: *"reduces the measured bracket, not the fault"*; (3) the analog
  coupling that drives (1). You've been polishing (2) while the fault is (3).

## The divergence: per-commutation TIM1 `EGR.UG`

Your E279 source audit noted `sixstep_write` resets TIM1 via `EGR.UG` on every
COM (`mzhal.rs:211`; `powered_timer.rs:478` "COM resets TIM1 via EGR"). I checked
the reference: **stock AM32 does not.** The only `EGR |= UG` in the entire g071
tree is on the *input-capture* timer (`Mcu/g071/Src/IO.c:38`) — never TIM1.
`phaseouts.c` and `main.c` have none.

Why it matters mechanically: `UG` reinitializes the counter to 0, so **the PWM
phase jumps at every commutation.** The COM timer fires asynchronously to PWM, so
each commutation re-phases the PWM against the *next* ZC's persistence window
differently every time — which is exactly "PWM-linked qualification variation" and
"crossing-time redistribution." It also truncates the in-flight PWM period at
each COM (a per-commutation pulse glitch). Your intent — "latch preloaded regs" —
dodges AM32's preload lag, but AM32 *lives with* that lag on purpose (CCR preload
lands at the next wrap, ≤1 period), made benign by `SET_DUTY_CYCLE_ALL` + a
GPIO-forced low leg. That trio was minz's "exact AM32 parity" fix and it removed
40% of its spike events. You appear to have the trio *plus* a per-COM `UG`.

So it fails the parity discipline on its own, **and** it's the strongest
mechanistic candidate for the jitter capping the envelope. **Bounded A/B:** drop
the per-COM `UG` (preload at the wrap, AM32-style; keep `SET_DUTY_CYCLE_ALL` +
forced low leg), rerun the 5.5% hold with the same near-fault tail, compare σ and
the E280 redistribution. A suspect with a mechanism and a reference divergence
behind it — not a proven cause. Test it first, before anything hardware.

## A second divergence: the 10 kHz carrier

E280's trace has PWM period = 100 µs → **10 kHz**. AM32 runs **24 kHz** on G071
(`TIM1_AUTORELOAD` 2665 at 64 MHz). Besides being a parity divergence, 10 kHz
*maximizes* the phase sensitivity: your 29–67 µs persistence window sits inside
**one** PWM phase at 100 µs, so every qualification samples a single phase of the
coupling. At 24 kHz (41.7 µs) the same window spans **0.7–1.6 periods** — it
averages across PWM phase instead of locking to one. Lever, with two caveats:
measure the M0 IRQ fraction after (more switching edges → more comparator
chatter to filter), and note the ON pulse shrinks proportionally (5.5% of 41.7 µs
≈ 2.3 µs) — check the DRV minimum pulse and any in-window sampling.

## The sense network — what the 47 k hunch actually costs

The network: MOTx → 82 k → VSENx → 7.5 k → GND (source Z ≈ **6.9 kΩ**), star of
three **47 k** legs to PA3, **no caps** (C2/C3/C4 DNP on the DRV, none on the star).

- **The 47 k was a good hunch.** Each VSENx sees 47 k to a node sitting near its
  own average, so the deviation-from-neutral is attenuated only to ~87% (~13%
  amplitude loss) and **the zero-crossing does not move** — loading through a
  resistor *to the neutral* can't shift the point where VSENx = neutral. 10 k would
  have loaded the 6.9 k node hard. So: no ZC error from the star.
- **Its real cost is a high-impedance neutral — and that's a trade-off, not a
  free win (the graybeard sold it as the latter; that was an omission).** The
  same choice that protects amplitude sets the neutral's impedance: star-center
  Thevenin Z ≈ (R + 6.9 k)/3 = **18 kΩ at 47 k** vs ~5.6 kΩ at 10 k — about
  **4× minz's** ~4.4 kΩ neutral (10 k star on 3.3 k dividers). A high-Z node is a
  better antenna: stray coupling from switching traces and free-wired jumper
  leads deposits more voltage on 18 k than 5.6 k. Which side of the trade-off wins
  depends on the operating point: at 50 eHz you were amplitude-limited and 47 k
  was right; at 297 eHz BEMF is strong and the limit is timing jitter, so the
  noise cost now shows. Note the "2×" divider impedance is the DRV board's
  82k/7.5k, not the star — nobody chose that. minz filtered the same coupling in
  software to 970 eHz on 3.3 k dividers and a ~4.4 k neutral. And the right cure
  for a high-Z node is a **cap on the node** (below), not a smaller resistor —
  10 k would buy noise immunity by giving back 41% of the amplitude.
- **If caps ever go on — match the time constants, or you'll shift the ZC.**
  minz's 4.7 nF recipe was for a symmetric 3.3 k/3.3 k network. Here INM (VSENx,
  6.9 k) and INP (star, ~18 k) are *asymmetric*. Rule: same τ on both. 4.7 nF on
  each VSENx gives τ ≈ 32 µs (fc ≈ 4.9 kHz, ~−14 dB at 24 kHz, ~3.5° at 300 eHz);
  the star center then needs **~1.8 nF** (32 µs / 18 k), *not* 4.7 nF — a 4.7 nF
  star lags the neutral (fc ≈ 1.9 kHz) and the delays no longer cancel at the
  crossing. Place them at the divider/star nodes, never at the MCU pins. This is
  a **hardware decision and it's the operator's** — do not solder on your own.

## CORRECTION (2026-09-14) — do not read this memo as "the hardware limits you"

The operator caught the graybeard drifting into a "hardware is bad" conclusion
on inference. Re-read the evidence: steady σ is **tight** (25–29 µs, ~0.8%) —
broadband analog noise would *raise* σ and doesn't. The remaining fault is a
**discrete outlier** (one cycle ~100–150 µs off, 4–5σ, otherwise silent), which
is the signature of a **timing/acceptance event** — a COMP accept delayed by a
preemption (the ~100 µs guard tick, DMA, UART), a stale read after the mux
switch, or a free-run/re-sync boundary — not noise. E280's "crossing-time
redistribution" is the tell. minz ran this same class of capless network to
970 eHz and deleted its caps A/B as unnecessary. Every prior wall on this rig
was code. **So: caps are OFF the table until stock AM32 is run on this rig.**
If unmodified AM32 holds high throttle here, the outlier is in the timing
layer and fixable. Investigate the outlier as a *scheduling* event: rerun the
fault with the near-fault tail trace ON and ask what preempted the persistence
window at that boundary. The priority order below stands only as history.

### Addendum (2026-09-14, same day) — "or UART talking, or something else nobody's looking at"

> **FALSIFIED (E478, later the same day):** binz wrapped the reference
> persistence loop in a critical section (31 060 calls, max 45 µs, zero guard
> overlap on the fatal call) and the 7.2% fault happened anyway. Mid-service
> preemption is *not* a necessary cause; the NVIC-order mechanism below is
> dead as stated. The priority table is still accurate as a parity divergence
> from AM32; it is not the outlier's cause. Stock AM32 then ran this rig to
> 741 eHz on known settings (E530) — see `GRAYBEARD_INSTRUMENT_DISCIPLINE.md`
> for where that leaves the investigation.

The operator's extension, and it's the right one. I went through the source
instead of theorizing. Here is **everything else that runs during a powered
hold**, with what I could verify (file:line) and what I couldn't.

**The NVIC map on the live powered path — this is the finding.** The M0+ has
two priority bits (levels 0x00/0x40/0x80/0xC0; your values are encoded
correctly, unlike rm32's old L431 bug). On the powered path:

| IRQ | prio | what | where set |
|---|---|---|---|
| TIM6 guard tick, **100 µs period** (PSC 63/ARR 99) | **0** | `powered_timer::interrupt` → `free{guard.poll}` | `powered_timer.rs:276-278` |
| DMA1_CH1 ADC feedback stream | **0** | `adc_stream::interrupt` → poll + `stream_current` + queue push | `adc_stream.rs:123` |
| **ADC_COMP (the comparator)** | **0x40** | persistence window + accept | `core_bench.rs:182` |
| TIM16 COM | 0x80 (0x40 with `bench-com-peer`) | commutation + `commit` | `core_bench.rs:183` |
| TIM7 polling | 0xC0 | | `core_bench.rs:270` |

**That is AM32's order inverted.** AM32-G071 runs COMP at the top and the
housekeeping ISR at the bottom precisely so nothing periodic can land inside
the persistence loop. Here two prio-0 sources — one of them *exactly 100 µs
periodic* — preempt the comparator ISR at will, including mid-persistence.
Mechanism, stated as a prediction so it can be falsified: the 12-read
persistence loop gets preempted by the tick (or the DMA ISR, or both
tail-chained); the reads resume tens of µs later at a different PWM phase;
the level has flipped (E280 — you already saw "rejects exactly one PWM
period apart"); the loop rejects; the ZC is then qualified on a *later*
carrier edge. **Prediction: the outlier magnitudes are quantized in carrier
periods — 2–3 × 41.7 µs = 83–125 µs at 24 kHz.** That is the "100–150 µs,
discrete, otherwise silent" signature, and it is not noise. You can test
this against data you already have: histogram the excursion sizes and look
for peaks at n × 41.7 µs. If they're there, it's a preemption/re-qualify
event; if they're smeared, I'm wrong.

Two cheap instruments to convict or clear it, no new hardware:
1. **Read `TIM6.CNT` at ADC_COMP entry** and log it with every accepted
   event. If the outliers cluster at CNT ≈ 0–20 (the ZC arrived just as the
   tick fired), the tick is the preemptor. Uniform → it isn't.
2. **Print the three maxima you already keep** next to the fault: `COMMIT_MAX_US`
   (`powered_timer.rs:378`), TIM6 `MAX_US` (`:561`), DMA `STATE.max_us`
   (`adc_stream.rs:64`). Their sum bounds the worst stacked delay a COMP can
   see. If the sum is under the excursion, stacking alone can't explain it and
   the re-qualify quantization above is the whole story.

The A/B, if the data says so: **COMP → 0, TIM6 + DMA1_CH1 → 0x40** (AM32's
order). One line each. The cost is the trade you must own, not me: the guard
tick can then be delayed by up to one COMP ISR (~65 µs) and `commit` already
trips `TickGap` at 100 µs (`powered_timer.rs:381`) — check the guard's
deadline slack before flipping, and keep the trip. Your comment at
`core_bench.rs:269` ("TIM6 stays at priority 0 and preempts this wait") was
written for the *forced-waveform* path; the powered path inherited it.

**The other things that run, checked:**
- **COM's `commit` masks all interrupts for the entire role write**
  (`powered_timer.rs:363-377`: guard.poll + blank/modes/sink/mux/enable). A ZC
  arriving inside is pended for `COMMIT_MAX_US`. Bounded, measured — cite it.
- **Foreground `service_feedback` drains up to 8 frames per call, each
  inside its own `interrupt::free`** (`powered_timer.rs:414-424` → `feedback_inner`
  `:394`). Eight back-to-back masked sections from *main* while the motor
  runs. Short each, but it's the foreground touching the ZC path's mask.
- **`bench-com-peer` puts COMP and COM at the same level** → no preemption,
  so a 69 µs COM blocks a COMP outright. Confirm which feature set the
  qualified build was linked with; "peer" is a probe mode, not a parity mode.
- **UART TX: I found no write on the powered path** in `driven_run`,
  `powered_timer`, `powered_guard`, `accepted_timing`, `comp_input` — every
  `writeln!` is post-run. Good; that's rm32's TIM16-print scar avoided. I did
  **not** audit all 2,000+ lines of `shell-pwm.rs` main loop — confirm nothing
  periodic (status, envelope, `t1u:`-style line) prints during a hold. At
  115200 a byte is 87 µs; a blocking TX anywhere in the window would be a
  candidate of exactly the right size.
- **UART RX: any received byte is an immediate abort** (`shell-pwm.rs:1128`
  `serial.read().is_ok()` → HostAbort=9). That's a *stop*, not a delay, so it
  can't be E383 (CycleTiming=12) — but make sure the host script is silent
  during holds; a keepalive would end a run and look like a refusal.
- **TIM7 (0xC0) and the ADC's own IRQ** — not on the interrupt-mode path;
  ADC events go through DMA, and ADC_COMP is the comparator vector only when
  `comp_input::filtered()` is false (`shell-pwm.rs:238`).

None of this touches the resistor network. All of it is code you own.

## Priority order — and why the caps are last

1. **`EGR.UG` A/B** — firmware, cheap, restores parity, biggest mechanism.
2. **Carrier → 24 kHz** — firmware, restores parity, de-phase-locks the
   persistence window. Measure IRQ fraction after.
3. **Only then, caps** — if measurable jitter remains after 1 and 2.

The order matters for a reason beyond cost: if you put caps on first, you'll mask
two firmware divergences with a hardware bandaid and never know which one fixed
it — and the A/B against stock AM32 (which has neither divergence and runs on
the same capless board) becomes uninterpretable. Fix the divergences, *then* ask
whether the hardware still limits you.

## Standing

The 5.5% trips are a jitter tail crossing a floor, not a speed limit — **don't
widen 3226.** Keep every guard. The cached-comparator work earned its keep as
CPU headroom; it is not the lever for this fault. And the current channel is
still uncalibrated raw peaks — that's the other open item nobody should forget.

— the minz graybeard
