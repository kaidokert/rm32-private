# firmware50 ↔ qualified binz image: closed-loop parity table

**Why this file exists.** firmware50 was built from the goal text rather than
from the qualified image, and four of its closed-loop constants turned out to
be my own fresh design decisions where the reference had a measured value
sitting in `binz/examples/support/` and `binz/AGENTS.md`. Each was reasoned
about from first principles, tested honestly, and written up — and each was
wrong, because none of these numbers are derivable. They were measured on this
rig and this motor.

The rule this table enforces: **clean shape, transcribed values.** The typed
policy composition is the improvement; the numbers inside the types are not up
for redesign until the port matches the reference. Every row is either
identical, or a divergence with a stated reason and a measurement behind it.

The single number that would have exposed all of it on day one:

| image | duty | speed |
|---|---|---|
| qualified binz (E762, 121 265 commits / 30 s) | 15% | **~670–704 eHz**, a locked rotor |
| firmware50 E024 (3/3) | 15% | **71–82 eHz commutation rate** |

**Read that second row carefully.** firmware50 has no rotor-speed instrument.
`ehz_from_sector` is derived from its own commutation count, so it measures how
fast this firmware switches the bridge, not how fast the rotor turns. Closing
the four divergences below raised that rate to 651–768 — and the operator
stopped the bench because the motor was grinding and very hot, i.e. the rotor
was *not* following. See LAB_NOTEBOOK E030 for the retraction and for the three
candidate speed witnesses that must exist before the next powered run. A rate
from this build may never be compared against the reference's speed until one
of them does.

`binz/DUTY_50_CAMPAIGN.md` carries the whole measured rung table: 10% → 401
eHz, 15% → 704, 20% → 941, 25% → 1186, 50% → 2096, with per-rung current proxy
and bus volts. **Every firmware50 run at duty X must have the qualified speed
at X written beside it.**

## Closed-loop path

| item | qualified reference | source | firmware50 | verdict |
|---|---|---|---|---|
| Gate map A/B/C | CH3/CH2/CH1, A PA10/PB1, B PA9/PB0, C PA8/PA7 | `AGENTS.md:8526`, `:8541` | same | ✅ match |
| BEMF sense | VSENA/B/C → PB3/PB7/PA2, star → PA3 | `AGENTS.md:8527` | same | ✅ match |
| Phase wiring | `bench-reverse-phases`: A↔B swapped, steps 1→4,2→3,3→2,4→1,5→6,6→5 | `examples/support/phase_direction.rs` | `commutation::Reverse` | ✅ match (since E010; was `Forward` — the bug) |
| Comparator mux | `INMSEL = 6 + floating`, `INPSEL = 2` (PA3) | `examples/shell-pwm.rs:452`, `examples/support/comp_input.rs:89` | same | ✅ match |
| COMP2 hysteresis | 0 (nonzero only under `bench-comp-hyst-low`) | `examples/shell-pwm.rs:1450` | `COMP2_HYST = 0` | ✅ match (E006 tried 2, reverted) |
| Post-mux settle | 10 µs (`FILTER_SETTLE_US`), 47 µs in the slow read path | `examples/support/comp_input.rs:11`, `examples/shell-pwm.rs:862` | `COMP_MUX_SETTLE_US = 10` | ✅ match |
| ZC detection | COMP2 → EXTI18 edge, per-sector `RTSR1`/`FTSR1` | `examples/support/comp_input.rs:91-99` | same, `ADC_COMP` ISR | ✅ match (since E022) |
| Startup carrier | 10 kHz | `examples/support/carrier_profile.rs:2` | `STARTUP_TICKS = 6400` | ✅ match |
| Run carrier | 48 kHz (ARR 1332) | `carrier_profile.rs:2`, `AGENTS.md:18` | `RUN_PERIOD_TICKS = 1333` | ✅ match (since E013) |
| **Handoff speed** | **50→200 eHz autonomous staircase** | `AGENTS.md:293` | **`HANDOFF_EHZ = 60`** | ❌ **mine.** BEMF at 60 eHz is a third of 200; the detector was being fed an eighth of the qualified signal |
| **Estimator bounds** | no fast-side band; only `commutation_interval < 50` → filter 2 | AM32 via minz-core | **`new_bounded(ci0, ci0/4, …)` → 240 eHz hard cap** | ❌ **mine.** E002 added it against a slow-side ratchet; it capped the fast side below the operating point |
| **Advance level** | **20 below 35% duty, 22 at/above**; scheduled advance clamped 18..22 | `AGENTS.md:133`, `:155` | **`ADVANCE_LEVEL = 26` at all speeds** | ❌ **mine.** 26 is the reference's *post-COM-wait override*, a different mechanism (`AGENTS.md:114`), not a scheduled advance |
| **Missed-edge policy** | comparator stays live and waits; `bemf_timeout` ends the attempt and restarts. No per-sector forced commutation in interrupt mode | AM32 / `AGENTS.md` restart entries | **forced commutation at `min(ci, 60 eHz grid) × 2`** | ❌ **mine.** A commutation a whole sector late is a braking event, on 39–46% of steps |
| Blanking | half cycle | AM32 `CNT > average_interval/2` | `REFERENCE_BLANK_64 = 32` | ✅ match (now a stated parameter, default = reference) |
| Level validation | inside the ISR, `getCompOutputLevel() == rising` | AM32 COMP handler | ISR-captured level | ✅ match (since E023) |
| Restart dwell | outputs disabled ≥1 s before restart; replay first-handoff settings | `NORMAL_RESTART_E770.md` (E768, E769) | script rests 3 s; no firmware dwell | ⚠️ partial |
| Acquisition / BEMF duty | acquisition 6.1%, BEMF 7.0%, phase +60° | `NORMAL_RESTART_E770.md` (E769) | `HANDOFF_DUTY_TENTHS = 70` (7.0%), no phase offset | ⚠️ partial |
| IRQ priorities | COMP/COM peers at 0x40 below 48% duty; COM → 0x00 at/above (`bench-com-top-high`) | `captures/reference/reverse_48k_com_top_high_20260919/README.md` | COMP at 0x00, no COM root | ⚠️ partial — COM/DMA/guard roots absent |
| Protections | 3-scan 5% fast-sag hard stop, nominal 4 A foldback, nFAULT, tracking, watchdog | oracle README | all present | ✅ match |

## Open divergences to close, in the memo's order

1. Estimator fast-side floor → physical maximum (~40 µs) or none; keep the
   slow-side bound.
2. Handoff at 200 eHz with the qualified staircase shape.
3. Advance 20 below 35% duty (not 26).
4. Missed-edge policy: live comparator + timeout-to-restart, not a late
   commutation on a grid.

Then, only if chatter remains at speed, binz's hardware comparator filter
(`bench-filter-control` / `filtered_irq_hw`), which also gives
hardware-stamped crossing times.
