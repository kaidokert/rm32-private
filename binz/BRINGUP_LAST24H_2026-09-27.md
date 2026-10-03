# The propless detour and return to AM32 alignment

Written 2026-09-27, America/Los_Angeles. Retrospective of the recent roughly
24-hour work arc, including earlier context needed to explain the detour.
This is a summary of retained records, not a new bench test or qualification.

## Bottom line

We did **not** obtain an improved, qualified high-duty controller from the
detour. We reached 70% propless briefly, with a protection stop, then spent a
substantial amount of effort on alternative switching and scheduling that did
not regain that operating point. The operator stopped that direction.

We subsequently restored the known loaded-60% controller, retained five
targeted safety/accounting/tooling backports, and tested two narrower AM32-style
processing changes. Neither demonstrated the required net timing benefit, so
neither was promoted. The installed firmware remains the restored baseline
with the five backports—not the diode or experimental scheduling controller.

The useful output is specific fixes, better evidence and rejected candidates.
It is **not** another qualified throttle increase, proof of an MCU ceiling, or
proof that AM32 alignment cannot work.

## 1. Propless exploration: real progress, then a failed 70% attempt

Removing the prop reduced noise and load, but changed speed, coast behavior
and the BEMF environment. Loaded qualification did not transfer automatically.
The exploration used a lean image with a staged carrier: 48 kHz at entry,
64 kHz above 35%, with COM-top selected in that experimental configuration.

| Record | Actual target dwell | Result |
|---|---:|---|
| E397, 60% | 14.778 s | Deadline completion; coast about 2926 eHz |
| E399, 65% | 12.278 s | Deadline completion; coast about 3139 eHz |
| E400, 70% | 9.025 s | FastBusSag stop; powered-tail estimate about 3292 eHz |

These were exploratory runs, not repeated qualification. The 70% run really
reached target, but it was not a clean sustained pass. Near-deadline “thin”
events increased markedly; their whole-run counts could not identify where
they occurred. Powered averages versus post-stop coast did not prove “slip.”

Source: [LAB_NOTEBOOK.md](firmware50/LAB_NOTEBOOK.md), E395–E400 and subsequent
retained fault investigations.

## 2. What the “diode thing” actually changed

The restored controller uses complementary PWM. During the source phase's
PWM-off interval, its low-side FET can conduct synchronously while the sink
low-side remains on. Under light load that permits reverse winding current.

The hypothesis was that disabling the source's complementary low-side drive
could improve this propless operating regime. Current would instead recirculate
through the diode when appropriate. The cached AM32 G071 source has an explicit
non-complementary option, but that did **not** establish that our successful
AM32 reference runs used it, or that it would cure our fault.

This was a change to electrical switching behavior, not just a faster ISR.
It changes losses, recirculation and the waveform presented to the comparator.
We did not measure reverse current well enough to establish it as the cause.

The investigation also expanded into:

- Preloaded/latching sector-role writes and controlled compare updates, to
  avoid partially applied register configurations.
- Fresh versus previously prepared waits and fixed-advance specialization.
- Timer-owned bounded level rechecks instead of depending only on foreground
  revisit service; generation ownership, cancellation and pending-IRQ handling.
- Persistence-loop code generation, protected acceptance/arming transactions,
  and a slower entry carrier followed by 48 kHz.

Each introduced additional ownership, timing or transition questions. Much of
the effort became proving the machinery around the experiment rather than
improving the operating envelope. Host tests, disabled pad checks and assembly
audits found useful defects, but did not prove energized waveform quality or
hardware WCET.

The final parked version passed a 15% screen (E488, 20.277 s at target), then
failed before reaching a 25% hold (E489). At the stop, applied duty was 23.931%;
one late arm recorded a requested 11 µs wait and 11 µs already spent. That is a
specific timing failure, not proof of a diode, supply or MCU fundamental limit.

The detour is parked on `codex/park-propless-e489`; E489's final image was
`0EE71575…`. None of that control architecture is installed now.
Source: notebook E415–E489, especially E416's rationale and E489's final review.

## 3. Return to the loaded baseline and fixes actually retained

The operator reinstalled a prop, fitted a different physical motor of the same
model, and boxed it for noise control. We restored the older complementary-PWM
controller rather than trying to repair the detour in place.

E490 completed 19.773 s at 50%; E491 completed 14.775 s at 60%. Both stopped on
their normal deadline with outputs off. The operator confirmed noise containment.
These were short acoustic/regression screens, **not renewed full qualification
of the replacement motor or thermal certification of the enclosure**.

Five isolated backports were retained:

| Backport | What it fixes |
|---|---|
| Stop-dominant foreground writes | A foreground operation must not re-enable/apply powered outputs after a guard stop |
| Stop/blank-dominant comparator resume | A delayed resume must not reopen the comparator after stop or during protected blanking |
| Fresh-sector revisit admission | Revalidate sector and COM-idle state inside the existing exclusion, not from stale foreground observations |
| Fail-closed assembly audit | Detect previously missed indirect/unresolved control transfers; host tooling only |
| Coherent accepted-event accounting | Count coalesced publications without inventing intermediate timestamps; defer inconsistent sequence/stamp reads |

Firmware-changing backports received individual short loaded-60% screens;
the host-only audit change did not trigger an unnecessary motor retest.
These fixes are not asserted to explain the earlier sag or late-arm failures.

Retained baseline: `binz-loaded60-backports-complete-20260927`, source
`039f046`, evidence commit `c4d4405`, ELF SHA256:

`236614731EB5E32BA293629A5F14E7D90F61BBA1D69C1B6CE28B80E1DCB2116F`

It keeps reverse direction, complementary PWM, 48 kHz running carrier,
advance 16, deep-filter policy and the existing protections.
Details: [SAFE_BACKPORTS.md](C:/Users/kaido/AppData/Local/Temp/binz-loaded60-restore/binz/firmware50/SAFE_BACKPORTS.md).

## 4. AM32 alignment attempt one: prepare the next wait in COM

The cached AM32 sequence qualifies a crossing and arms a previously prepared
wait; COM then commutates and prepares the next estimate/wait. Our baseline
updates the estimate and calculates the current wait on the COMP path.
Their timestamp origins also differ. Moving calculations is therefore not
automatically a behavior-preserving refactor.

We implemented COM-owned preparation with accepted-event/sector ownership and
kept same-event protection information through a post-arm preview. Tests and
disassembly established that estimator arithmetic moved out of the pre-arm
path. However, publication, validation and protection work added costs elsewhere.
An initial change also altered blanking eligibility; that confound was corrected.

Corrected E502/E503 local timing measurements showed:

| Entry-to-post-arm bracket | Baseline | Candidate |
|---|---:|---:|
| Median | 8.500 µs | 8.500 µs |
| p95 / maximum | 11.125 µs | 11.250 µs |

Both short screens completed, but the required latency improvement was not
demonstrated. We rejected promotion and skipped the long qualification cohort.
This does not prove that a leaner AM32-order implementation cannot improve it.

Parked branch: `codex/loaded60-am32-order`, final record `8e47f0c`; tag
`binz-loaded60-comprepare-rejected-20260927`.
Details: [AM32_ORDER_CAMPAIGN.md](C:/Users/kaido/AppData/Local/Temp/binz-loaded60-restore/binz/firmware50/AM32_ORDER_CAMPAIGN.md).

## 5. AM32 alignment attempt two: COMP/COM above ADC DMA

We retained the safety guard at highest priority and tried lowering DMA below
COMP/COM. The audit found a real prerequisite: a preempted DMA handler could
copy a circular ADC source while hardware overwrote it. A publication sequence
counter alone does not protect that source buffer.

The candidate added ping-pong acquisition, inactive-half/epoch validation,
sticky invalidation and conservative completion-age publication. Disabled
collision/fault tests and reviews exercised it without masking the whole DMA
handler. Foreground delivery counters also exposed skipped publications; a
healthy producer is not proof every scan reached the protection consumer.

After a production short screen, three diagnostic conditions were compared:
the original tagged baseline, the repaired ADC adapter at peer priority, and
the repaired adapter with lower DMA priority.

| Software-deadline-to-post-bridge estimate | Tagged baseline | Repaired peer | Lower DMA |
|---|---:|---:|---:|
| Median | 3.750 µs | 3.750 µs | 3.750 µs |
| Nearest-rank p95 | 4.875 µs | 6.750 µs | 5.750 µs |
| Observed maximum | 6.750 µs | 7.750 µs | 7.375 µs |

These are single-run terminal tails of roughly 25–27 ms, not whole-run WCET.
The estimate retains coarse quantization and omits the gap between two clock
reads. It is not a hardware-expiry or electrical-switching timestamp.

Lower DMA improved part of the repaired adapter's tail, but showed **no
demonstrated net benefit over the original baseline**. Both independent
reviewers recommended rejecting promotion. No long qualification runs followed.

The candidate remains isolated on `codex/loaded60-dma-priority`; final record
`f72c632`. Exact baseline was restored with verified flash/reset, fresh
outputs-off PASS and UART closure at 02:59:46 UTC Sep 28 (Sep 27 locally).
Details: [DMA_PRIORITY_OUTCOME.md](C:/Users/kaido/AppData/Local/Temp/binz-order-baseline/binz/firmware50/DMA_PRIORITY_OUTCOME.md)
and [campaign/reviews](C:/Users/kaido/AppData/Local/Temp/binz-order-baseline/binz/firmware50/DMA_PRIORITY_CAMPAIGN.md).

## 6. What we learned—and what we should stop doing

The cost was disproportionate to envelope progress. The propless campaign
branched into switching topology, timer retry machinery, synchronization,
instruments and repeated low-duty admission work. The best duty number did not
advance while those layers accumulated. That is a process failure to recognize,
not something to disguise as qualification progress.

Concrete assets survived: the five backports, source-level concurrency findings,
the ADC coherence/freshness design, and corrected measurement code. We caught
timestamps placed after bookkeeping rather than immediately after arming, mixed
coarse/fine service units, and a deadline read after the timer had been rearmed.
Earlier affected bridge/angle outputs must not be revived as valid evidence.

Going forward, AM32 alignment should mean **less necessary work on the deadline
path**, not moving the same work and surrounding it with more machinery. Start
with a bounded offline state/order audit and an explicit emitted-code saving.
Then test the smallest candidate that preserves event identity and protections.
Do not redesign the controller merely because one implementation failed, and do
not qualify an experiment whose intended benefit has not appeared.

We still have a working loaded-60% baseline. We have not established a hardware
ceiling, a need for hardware commutation, or a viable path to 100% by this work.
The current architecture comparison and constraints remain in
[AM32_ALIGNMENT_PLAN.md](AM32_ALIGNMENT_PLAN.md).
