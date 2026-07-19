# COMP2 full-revolution sampling — diagnostic capture plan

## Goal

Capture **everything that happens on COMP2 over one electrical
revolution**, dump it on demand (key press / RTT command), so we can
look at the raw zero-crossing waveform vs the commutation timing
instead of inferring through ISR-side counters (`raw` / `valid` / `filt`).

Same spirit as the bit-bang UART RX capture we did earlier — small
ring buffers in SRAM, latched by ISR, dumped from main.

## Sizing math

System clocks / known rates:

- SYSCLK = 80 MHz
- PWM = 24 kHz (`PWM_FREQUENCY_HZ` in `src/lib.rs:31`) → period 41.6 µs
- Slowest useful bench speed: F_elec ≈ 30 Hz (below that the rotor cogs)
- Fastest open-loop V/f we can hold cleanly: F_elec ≈ 200–300 Hz

PWM samples per electrical revolution:

| F_elec | PWM cycles / rev | Bit-packed bytes |
|--------|------------------|------------------|
| 30 Hz  | 800              | 100 B            |
| 50 Hz  | 480              | 60 B             |
| 200 Hz | 120              | 15 B             |

A **128-byte (1024-bit) buffer** covers the worst case (30 Hz) with
~30 % margin and rounds to a power of two for cheap index math.
At higher speeds, the buffer covers more than one revolution; that's
fine — we can either ignore the tail or wrap the index.

## Two buffers, two viewpoints

### Buffer 1 — PWM-synchronous sample

ISR fires at the PWM rate (TIM1 update or CH4 TRGO).
Each fire: read `COMP2.CSR.VALUE` and shift into bit `N` of `PWM_BUF`.

```rust
static PWM_BUF: [u64; 16] = [0; 16];  // 128 B, forces wide stores
static PWM_IDX: AtomicU16 = AtomicU16::new(0);
```

Hooked on TIM1 CH4 specifically (rather than UPDATE) lands the sample
at the **center of the high-side ON-time**, where the gate-driver
coupling on PB4 (virtual neutral) is most settled. AM32 doesn't have
a precise equivalent; this is a strict upgrade for diagnostic quality.

No clear needed — each capture window overwrites in place.

### Buffer 2 — Event-binned

COMP ISR fires on every EXTI line-22 edge.
Each fire: compute `bin = (TICKS_10US − SECTOR_START_TICK) >> SHIFT`,
set that bit in `EVT_BUF`.

```rust
static EVT_BUF: [u64; 16] = [0; 16];  // 128 B
```

`SHIFT` chosen so `period_ticks >> SHIFT` ≈ 1024, giving a
speed-adaptive **"one revolution view"** at constant 1024-bin
resolution regardless of rotor speed. Each bin is `period / 1024`
wide — at 30 Hz that's 32 µs/bin, at 200 Hz it's 4.9 µs/bin.

This buffer **must be cleared** every revolution (we OR bits in;
stale bits would survive). See "memclr cost" below.

## memclr cost (the question that prompted this doc)

Cortex-M4 @ 80 MHz, 128 B = 16 × u64:

- Best case (STRD or paired STR via LDM/STM): ~20 cycles ≈ **0.25 µs**
- Typical `[u64; 16] = [0; 16]` or `slice.fill(0)`: ~60–80 cycles ≈ **1 µs**
- Worst case (bytewise `compiler_builtins::memset`): ~250 cycles ≈ 3 µs.
  Avoidable by making the buffer `[u64; N]` so the compiler emits
  wide stores rather than calling the builtin.

Compared to existing budgets (from `port_41.log` heartbeat):

| Window                       | Budget  | 1 µs memclr = |
|------------------------------|---------|---------------|
| Electrical rev @ 30 Hz       | 33.3 ms | 0.003 %       |
| PWM period (24 kHz)          | 41.6 µs | 2.4 %         |
| TIM6 tick (50 µs)            | 50 µs   | 2 %           |
| Commutation ISR (TIM14)      | ~8 µs   | 13 %          |
| COMP ISR (BEMF)              | ~0.5 µs | **don't**     |

**Where to put the clear:**

- Not in COMP ISR — that's our hot path, 1 µs would triple it.
- The "step 0 wrap" moment in the commutation tick is logically the
  right boundary, but it adds 13 % to that ISR.
- Best: **double-buffer**. Two `EVT_BUF`s, ISR writes the "active"
  one, main-loop atomically swaps pointers at end-of-rev and clears
  the inactive one outside any ISR. memclr cost vanishes from the
  ISR budget entirely.

## Dump path

Add a UART key (`L` for "log"?) that:

1. Atomically snapshots both buffers + the current sector index.
2. Re-prints them as 1024-char ASCII strings (`.` = 0, `#` = 1) via
   the existing `dprintln!` UART. 1024 chars × 2 buffers = ~2 KB of
   UART → ~180 ms at 115200 baud. Acceptable for one-shot.

Optionally also dump the `current_sector` history and `SECTOR_START_TICK`
values so we can correlate the captured edges with which 60° window
they fell in.

## Phasing — what to build first

1. **Buffer 1 only**, hooked on TIM1 UPDATE for simplicity (not CH4
   yet). Verify the dump path works end-to-end with a known signal
   (e.g., spin at amp=15, F=80 Hz, observed_phase=A; the buffer
   should show two ~equal-length clumps of `#` per rev).
2. **Switch hook to TIM1 CH4** to sample at high-side ON-center.
   Compare cleanliness of the trace vs UPDATE-sampled. If meaningfully
   cleaner, keep CH4; if not, UPDATE is fine.
3. **Add Buffer 2** with double-buffering. The trigger to flip +
   clear is "entering sector 0 from sector 5" (i.e., commutation
   step wraps).
4. **Optionally correlate** with a third small log: per-rev counts
   of `raw` / `valid` / `filt` snapshotted alongside, so we can see
   "this captured rev produced these stats."

## Open questions

- TIM1 CH4 already drives TRGO (`CCR4=TIM1_CCR4_TRGO`) used by ADC
  triggering on AM32. Do we have an unused EGR/IRQ on CH4 we can
  bolt the sample ISR onto without disturbing TRGO? `CC4IE` is
  independent of TRGO selection, so yes — enabling CC4 interrupt
  doesn't break anything.
- Verify `is_multiple_of` / shift math doesn't cost more than the
  memclr we just optimized away.
- Do we want the PWM-sampled buffer indexed by **PWM cycle within
  electrical rev** (resets each rev, gives a per-rev histogram view)
  or **free-running** (rolling 1024-sample window, easier to capture
  on a chop event)? Probably the latter for chop diagnosis, former
  for "what does a clean rev look like."
