# krabilorean — L431 (Cortex-M4) arithmetic-personality fitness qualification

**Goal:** On the minz L431 bench, produce a complete, decisive **M3/M4 fitness
dossier** for krabilorean's arithmetic personalities at tag `v0.1.0-alpha.3`
(commit `e3b4466`) — settling *on real silicon, with the real firmware
consumer, on real commutation data* whether **WideExact is the M4 product
personality**, and proving that the const planners fold, the narrow paths stay
wide-helper-free, and the shared-artifact retained profile survives end-to-end.
Explicitly **bounded to M4**: the decisive M0 exact-vs-block experiment is out
of scope here (needs F0/G0 silicon) — this bench closes everything *except*
that one crux, so the M0 board is the only remaining unknown afterward.

**The decision it resolves:** the agent's own warning is that block scaling
isn't automatically faster and *exact currently wins decisively on M3/M4*. This
bench converts "currently wins (modeled)" into "wins (measured, on this L431,
with the whole-event pipeline)" — or refutes it. Output is a go/no-go on
WideExact for the M4 field profile.

## In scope — what this bench delivers (mapped to the campaign matrix)

| Area | M4 evidence produced |
|---|---|
| Numerical accuracy | every personality (WideExact, BoundedExact64, BlockScaled32<8/12/16>, both rounding policies) replayed over identical real captures — lossless paths **bit-identical**, every lossy result **inside its published bound** |
| Semantic stability | period-peak / harmonic / Gaussian-MI-minimum / zero-variance / trigger **decision flips logged separately** from raw Q-format error |
| Wide compile-out | objdump/nm gate on the *reachable* firmware call graph — assert **no u128 helpers and no forbidden `__aeabi_l*` 64-bit mul/div/shift** where a 32-bit personality claims their absence |
| Const planning | const-plan == runtime-plan, valid plans compile-fold, rejected plans fail before processing — across stable/nightly, MSRV/current, LTO on/off |
| Physical timing | DWT.CYCCNT **min/median/max over many reps** per personality×workload, vs the QEMU-instruction and analytical envelopes (DWT stands in for J-Trace on M4) |
| Integration | **detection → capture → retained evaluation → frame emission** on the live motor stream — end-to-end cost, painted-stack high-water, epochs, transactional rejection |
| Resources | flash, reachable code, static state, workspace, painted stack → the **exact-vs-narrow Pareto table** for the production config |

## Workload priority

The whole-event firmware pipeline and the **`BlockRetainedCaptureProfile`
end-to-end** (campaign #5 — the shared-artifact profile with *no physical
qualification yet*) are the headline, because those are exactly what only a real
spinning bench can exercise. BlockMoments32::update / centered+variance /
periodicity / Gaussian-MI are the per-primitive rungs underneath.

## Adversarial inputs

Run alongside the real captures: large DC offset with one-bit variation, and
constant/near-constant windows that may quantize to zero variance.

## Non-goals (this bench)

The M0 hardware exact-vs-block result; promoting LimbExact32 (M0-gated);
Stage-G examples / release closure.

## Why the fresh battery matters

Physical timing wants many repetitions and integration wants a *sustained real
lock* generating the actual event stream — both need the motor spinning
steadily, kill-guarded, without supply sag confounding the cycle distributions.

## Deliverable

A `krabilorean` **M4 fitness dossier** — the Pareto table, the decision-flip
log, the compile-out gate result, the cycle distributions, and the end-to-end
pipeline verdict — rendered as a study artifact + markdown field report, feeding
straight into the "which personalities are products" decision for everything M4.

---

*Bench: L431 @ 80 MHz (thumbv7em) · consumer: minz `krabimon` feature +
`krabilorean-probe` host harness · target crate: krabilorean v0.1.0-alpha.3.*
