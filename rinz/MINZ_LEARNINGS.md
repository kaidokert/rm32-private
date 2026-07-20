# minz → rinz: transferable learnings (banked 2026-07-19)

Distilled from the minz (Vimdrones L431, comparator-based) campaign that reached AM32
operating-map parity — rung 100, true-100% duty, ~2400 Hz, walk-proof by construction
(`ba2bbc2`, `2be5ae2`). Commit hashes are anchors into the shared repo history; the full
detail lives in those commit messages. Apply to the rinz G431 (ADC valley-sampling) restart.

## 1. Architecture — the rotor owns the clock

The minz arc converged on AM32's actual shape. Port the STRUCTURE to rinz; don't re-derive.

- **Scheduling inversion** — the rotor owns the commutation clock (`7a4b77f`). This is
  rinz's unsolved free-running-PI problem, solved and proven on this motor class.
- **`TIM1.UIE` OFF** — the CPU never sees the PWM carrier; all wrap work lives in a
  ~20 kHz control-loop timer; commutation on a dedicated high-priority timer vector
  (`97e8b0b` wholesale convergence, `010fdee` TIM16 COM timer free-run+ARPE+UG).
  Note the arm-on-accept vs always-re-arm inversion: unparked free-run = commutation storm.
- **Schedule-first, estimator-after** in the ZC accept path; diagnostics dispatched after
  accept (`b7afb8e`, `412db65`) — cut accept latency 16.5 → ~7 µs, moved the schedule wall
  from ~1.8 kHz to ~3.5+ kHz.
- **Featherweight commutation path** — stamp/fabric split, AM32-shape (`4a67f8f`).

## 2. The advance law — rinz has this WRONG today

- **Speed-keyed advance is POSITIVE FEEDBACK** (`672ed17`): estimate falls → advance rises
  → earlier commutation → compounds into the subharmonic walk. AM32's law is **duty-keyed**
  (`map(duty, 100, 2000, 13, 23)` degrees) — walk-neutral feed-forward.
- rinz's `--adv-sched` ramps advance off commanded Hz. Near-harmless in governed mode
  (Hz pinned) but becomes exactly this feedback loop the moment the estimator drives.
  **Flip to duty-keyed before any free-running work.**
- Also: gate advance out of the engage transit (advance-neutral start, AM32 does the same).

## 3. Instrumentation traps rinz is currently sitting on

- **busy-% is unreliable** — boot-sensitive baseline, read from the starving context;
  minz ripped it out (`1282219`, `08d2710`). rinz's `cpu_busy` idle-loop metric is the same
  primitive. DWT per-ISR cycle durations are the authoritative instrument (rinz's `isr_cyc`
  is already the right kind).
- **Flash prefetch/wait states change hot-ISR timing with LAYOUT** (`8d32e66`, `adb676f`):
  prefetch off = innocent code motion changes ISR cost run to run. On G431 bringup, check
  FLASH.ACR (wait states + PRFTEN/ICEN/DCEN) FIRST, before believing any cycle measurement.
- **Match-arm locals merge into main's prologue frame** (`60920dd`; LTO exposure
  `02bcf22`/`55fb4d4`): big branch-local buffers reserve stack at fn entry, silently overlap
  .bss (no guard), masquerade as robustness collapse / "LTO breaks it". Fix: extract big
  dump bodies `#[inline(never)]`; verify with objdump `sub sp` vs RAM−bss. rinz's
  scope_cl2_48k main loop has big dump bodies in match arms — AUDIT NEEDED.
- **Spin ban** (`a94a4aa`): every unbounded hardware-flag wait is a wedge candidate.
  Bounded `spin_until(max, closure)` + timeout counters; audit ISR-reachable waits first.
- **SysTick at high rate is a hidden CPU tax** (`3037876`); DWT.CYCCNT is the wall clock
  (`4fd43f1`), with wrap-extension store-order care (`89daf5e`).
- **Preflight toggle reset** (`7bc8f6d`): stateful runtime toggles persist through kills —
  only reflash resets. rinz hit this exact class (the mystery "not-catching" runs).
  Scripts must probe-write every toggle to known state before EVERY run.
- **Instrument decisions, not outcomes**: every silent veto path gets a per-kind counter at
  birth (`0292b6b` rejection census); outcome metrics are blind to refusal-class failures
  (the estimator INT_MIN clamp hid behind 100% qzc for days, `072a283`).

## 4. The safety stack — what would have saved the dead G431 board

rinz's scripts ran `stall_kill=False` while energized stalls cooked the A leg. The new
board does not see power until the equivalent of minz's stack is in:

- Firmware: baseline **sag-kill with debounce (8 samples/1.3 ms) + latched trip value**
  (`49c993d`, `b64c4e9`), throttle envelope, IWDG.
- **COMP/detector-storm mask-and-kill** with per-window edge budget (`a88faa4`).
- **Main-starvation guard** — shed telemetry at 250 ms staleness, clean kill at 500 ms,
  before the IWDG (`08d2710`).
- Host: **hard vbat abort** in every ladder/sweep script (`58cd73e`); **kill guard (`w`) on
  every script exit path** + current-based stall abort (~800 mA idle-stall signature).
- **`.uninit` flight recorder** surviving reset (BEACON_MAIN/ISR/PHASE printed at boot) —
  the starved-vs-wedged discriminator (`a056cd8`, `2e52a90`).
- Comm-silence backstop + bounded UART writes (TX_DROPPED, kills the telemetry-wedge
  reboot class) (`89daf5e`, `9b9490c`).

## 5. Methodology — what actually cracked the hard bugs

- **Paired ABAB verdicts**, revert-with-verdict recorded in the commit; single runs are
  anecdotes (run-to-run variation is large).
- **BENCH NEVER DRIFTS**: a failure is a code cause to bisect (flash known-good tag,
  confirm, bisect). "Warming"/"drift" narratives were wrong every time they were used
  (`045b1e0` purge, `715097d`).
- **Align quantities with the reference** (`bc4658e`): log the reference's EXACT fields,
  use them ITS way, plot side-by-side until they line up. Flattened a +54 µs parity bias
  in one histogram after 7 blind experiments. For rinz: capture AM32/minz-format traces
  and plot rinz on the same axes (the operating-map format is the reference curve).
- **Dwell-averaged telemetry, not instantaneous samples** (`be7f849`): the operating-map
  "jaggedness" was 100% sampling artifact; averaged curves were glass-smooth. Firmware
  accumulators (means per rung/plateau), not point reads.
- **Per-polarity / sector-parity ZC displacement** (`a8c8c18` ±1 carrier period by BEMF
  polarity; per-polarity delay trim `f94fa50` → first zero-reseed ladder). Close cousin of
  rinz's unexplained ±48° per-sector wave — re-examine with this lens on the new board.
- **Prove the reference exercises the path** before comparing failure handling (AM32's
  recovery = dead code; it never misses).
- Save a raw capture per plateau; `cl_spin_look`-style pre-flight health check (all three
  voltage staircases + all three currents) at the start of every bench session.

## Application order for the G431 restart

1. **Bringup checks** — FLASH.ACR wait states + prefetch, NVIC priorities, stack-frame
   audit (`#[inline(never)]` big dump bodies), spin-ban audit.
2. **Safety stack + script hygiene** — sag-kill/IWDG/storm guards in firmware; kill guards,
   vbat abort, preflight toggle reset, per-plateau raw capture in scripts. BEFORE first spin.
3. **Advance law** — flip speed-keyed `--adv-sched` to duty-keyed (AM32 map), engage-gated.
4. **The real port** — AM32-shape scheduling inversion (rotor owns the clock) for the
   free-running loop; `cl_spin_look` + align-quantities-with-reference as standing
   instruments; minz operating map as the reference curve to plot against.
