# Graybeard memo — renegotiate the parity target (2026-09-13)

binz, this memo is different from the others: it's not advice on the bench, it's a
prompt to **go back to the operator and renegotiate what "parity" means.** The
original objective — "reproduce minz bar-for-bar on the G071, then fold into
rm32" — has been read as *match minz's numbers*. The evidence you've generated
says that reading doesn't fit the silicon. That is not a failure. It's a scope
fact, and the operator is the one who sets the target — so bring it to them.

**Nothing in this memo is a target.** Every number below is evidence or a
framing option. The operator decides.

## What the evidence says (regime-labeled)

- **Qualified point:** 5.3% / ~284 eHz — 10 s hold, fixed 30 s recovery cohort
  **3/3**, σ ~45 µs, COM ≈ accepted every run (E268–269).
- **The wall, found from two directions:** the range-300 cycle floor (5.4% trips
  an exact 3331 < 3333 µs, E270) and the **recovery scheduling constraint** —
  seed age 212 + 64 arm = 276 half-µs → **~302 eHz** (E270). Coincident, both
  timing. IRQ-union went **52% → 60.5%** from 235 → 284 eHz; extrapolated, CPU
  saturates near ~470. The recovery-arm floor is the binding one.
- **minz's reference band starts at 528 eHz** (E034) — on a **Cortex-M4 @ 80 MHz**
  with DWT, hardware wide-multiply, and an M4 ADC/injected-group architecture.
- **You are on a Cortex-M0+ @ 64 MHz**: no DWT, soft `__aeabi_lmul`, a 1-byte
  USART RDR, tighter everything. Reaching 528+ at current per-call cost needs
  ~2× the speed ⇒ roughly *halving* per-call ISR cost. Know the size of that ask
  before anyone signs up for it.

**This is NOT a declared wall.** It's the measured trajectory at the current
per-call cost. minz's own rule applies: don't call something impossible until
it's stress-tested against the reference. Which brings us to the reframe.

## The reframe that makes the target honest

**For a G071 qualification stack, the right reference is stock AM32 *on the
G071*, not minz on the L431.** minz was a research vehicle on different silicon;
its 528+ band is an M4 artifact, not a property of the control science. rm32's
*production* G071 targets (flycolor, iflight, tbs …) run AM32 on this exact M0+.
So the parity that actually matters for the qual stack is:

> *Does rm32-G071 / this bench match stock AM32-G071 on identical hardware, in
> AM32's own quantities, at whatever band this silicon supports?*

The other agent's `DRV8304H_G071` build makes that reference **available for the
first time** — same board, same wiring, same M0. That's a far cleaner yardstick
than an L431 rig that isn't even on the bench.

## Candidate targets — options for the OPERATOR, not gates

**A. Quality parity vs stock AM32-G071, same hardware.** Compare in AM32's
quantities — cycle σ, accepted-ZC (qzc) rate, recovery success rate, dropout
rate, transient/startup current — at the M0-supported band. **Lock retention
first, speed second** (the operator's stated ranking). Speed becomes a
*characterized ceiling*, not the score.

**B. Ceiling characterization as its own deliverable.** Where does the M0 run
out (recovery arm, cycle floor, IRQ), and — the decisive question — **does stock
AM32 hit the same ceiling on this board?** If yes, it's the *platform* ceiling
and parity means "we match it." If stock AM32 goes meaningfully higher, you have
headroom to find and the ~302 is yours, not the silicon's.

**C. The portable-improvement inventory.** The objective's second half — "fold
demonstrated improvements into rm32" — is already substantial and doesn't depend
on speed at all: the timing-budget techniques (staging before the edge, yield on
ownership loss), the recovery/guard/revocation architecture, the polarity
convention fix (E029/E072), the EXTI-storm rate guard (E094), the USART3-clock
landmine, the sampled-clock extension. Catalogue it. It may be the most valuable
thing this bench produces regardless of where the eHz lands.

If the operator wants a **speed** target, it should come from **option B's
measured A/B ceiling** — not from minz's L431 band.

## What to bring to the renegotiation

1. **The stock-AM32-G071 A/B at matched conditions** (once `UART_DUTY_MODE` is
   ported to USART3/PC10–11) — the ceiling test *and* the quality baseline.
2. **Your quality metrics at 5.3%/284 in AM32's quantities**, so A vs stock is
   an apples-to-apples table.
3. **The ceiling analysis** (you already have it — E270/TIMING_HEADROOM).
4. **The portability inventory** (option C).
5. **A proposal, not a decision:** lay out A/B/C with the evidence and ask the
   operator which one — or which combination — is the target. Don't pre-select.

## Net

Redefine parity from *"minz's speed"* to *"AM32-G071's behavior on this silicon,
quality-first."* Bring the evidence, propose the options, and let the operator
set the number. Whatever it is, it'll be a target the platform can actually meet
— which is the only kind worth chasing.

## Other points of wisdom — the ones that outlast any target

Standing rules from minz/rm32, each paid for. Most you already practice; the
value is having them written down for when target pressure arrives.

1. **The bench never drifts.** A result that changes is a *code* cause to bisect
   — flash the known-good tag, confirm it still passes, then bisect. Keep a tagged
   known-good build per qualified operating point (you already freeze reference
   archives; make the tag the reflex). Never attribute a regression to "the
   bench." minz lost days to that crutch; you found the CCMR and TIM2-CEN bugs
   precisely because you didn't reach for it.

2. **Fix the cause, never the threshold.** Every memo has said it; here's the
   reason it matters most *now*: once the operator sets a target, the fastest
   way to "hit" it is to widen a guard. Every guard you've built (32 µs arm
   floor, cycle floor, `FLYTooSlow`, IRQRATE) was right when it refused. If a
   target needs a guard loosened, the target is wrong, not the guard.

3. **Supply is never the wall — and measure the power path first.** Every
   "supply wall" minz ever saw was a loop-created transient surge; a bigger PSU
   just feeds the heater. And a separate, recurring physical fault: power-path
   *resistance* (a connector melted at ~0.65 Ω / 7 A on the L431 rig). Your bus
   floor is already ~0.6–0.9 V below supply. Before you climb duty: measure the
   sag-vs-current slope, and never qualify through a path ≥0.3 Ω. A sag death is a
   "transit-surge defect" to hunt in the loop — after you've ruled out copper.

4. **Constant per-tick ISR cost.** Keep interrupt work *uniform*. Never branch
   heavy work onto commutation ticks to lower the *average* — spikes raise the
   worst case, and worst case is what trips a 32 µs floor. Your timing-budget
   work should optimize the worst-case bracket, not the mean.

5. **Instrument decisions, not outcomes.** Every silent veto/refusal path gets
   a per-kind counter at birth (you do this — reason codes, retained refusals).
   Outcome metrics like qzc are blind to refusal-class failures; the TIM2-CEN
   bug hid behind a plausible-looking outcome until CORESTATE exposed the
   *decision*. Keep it that way as the code grows.

6. **For the A/B specifically — two rules that decide whether it's valid:**
   - **Prove the reference exercises the path.** Before comparing recovery or
     desync handling against stock AM32, *count in the reference* that it actually
     hits that path at your conditions. If stock AM32 never desyncs at 284, its
     recovery is dead code — comparing your recovery quality to it is meaningless.
   - **Align quantities with the reference.** Log AM32's *exact* quantities
     (`average_interval`, `zero_crosses`, its desync operands), use them *its*
     way, and plot side-by-side until they line up. minz cracked a week-long
     parity bias in one histogram once the quantities were aligned. Don't
     compare your derived metric to its raw one.

7. **Regime-scope every verdict.** Label each conclusion with the speed, duty,
   build, and supply it was measured in. "Not a lever" verdicts *expire* — advance
   was useless at 400 Hz and worth +108 Hz at 900. You already do this; keep the
   habit when someone quotes an old result at a new operating point.

8. **Make the persuasive artifact.** For the renegotiation, don't bring a wall
   of log entries — bring a lock map and a dropout plot for your qualified point,
   the A/B table in AM32's quantities, and the ceiling curve. Board-internal
   instrumentation beats a scope for *why* questions, and one honest figure moves
   a decision faster than fifty retained captures.

**The asset is the discipline.** Append-only, refusals retained, no over-claim,
safed every run, bisect-not-blame. That's what got a G071 M0+ to a 3/3
thirty-second recovery cohort. Protect it hardest exactly when the pressure to
hit a number is highest.

— the minz graybeard
