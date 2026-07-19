# BEMF_50 Design v2: Virtual Neutral + Zero-Cross Display

Supersedes v1. Incorporates BEMF_50_DESIGN_v1_REVIEW1 and REVIEW2.

---

## 0. Milestone Scope (What This Is and Is Not)

This document specifies a **display/debug experiment**, not a zero-cross detector.

The deliverable is:
> An honest, visually aligned per-sector BEMF-vs-neutral display in the `e` command that
> makes ZC timing errors directly readable by eye.

Promotion to a real detection ISR requires a separate design phase. Nothing in this
document should be interpreted as a detection specification.

---

## 1. Display Representation: 8-bit / 4-bit Right Shift

All ADC hex displays shift the value right by 4 bits before printing — display only,
no effect on stored values or calculations. Applies to `l`, `m`, and `e` commands.
Not applied to the `n` register-state dump.

```
Raw 12-bit: 0x06A1  →  Display: 6a  (2 hex digits, v >> 4)
```

---

## 2. Channel Mapping

DMA buffer: `ADC2_DMA_BUF[3] = [ch5/B, ch14/C, ch17/A]`

| Phase | Index | Channel | DMA slot |
|-------|-------|---------|----------|
| A     | 0     | ch17    | 2        |
| B     | 1     | ch5     | 0        |
| C     | 2     | ch14    | 1        |

```rust
const PHASE_TO_DMA: [usize; 3] = [2, 0, 1];
const HIGH: [usize; 6] = [0, 0, 1, 1, 2, 2];
const LOW:  [usize; 6] = [1, 2, 2, 0, 0, 1];
// float_phase[s] = 3 - HIGH[s] - LOW[s]
```

Per triggered DMA sample in sector `s`:
```
V_hi   = DMA_BUF[ PHASE_TO_DMA[HIGH[s]] ]
V_lo   = DMA_BUF[ PHASE_TO_DMA[LOW[s]]  ]
V_neut = (V_hi + V_lo) >> 1
```

---

## 3. Virtual Neutral — Computation and Accuracy Caveats

### 3a. What it is

`V_neut = (V_hi + V_lo) >> 1` is a **pragmatic reconstructed threshold**, not a proven
physical star-point measurement. It uses the same ADC2/DMA path and the same divider
family as the BEMF signal, which is the best available on this hardware.

### 3b. Known limitations (do not paper over these)

**Divider non-linearity:** The BEMF front-end is a clamped single-ended divider. There
is no guarantee of perfect symmetry between the three channels. A systematic bias in
V_neut will shift every ZC index by a fixed number of samples — this looks convincing
but is wrong.

**Sequential, not simultaneous, sampling:** The triggered scan captures B → C → A in
sequence (~14 ADC clocks each at 42.5 MHz ≈ 1 µs total). V_hi, V_lo, and V_fl are
not captured at the same instant. If the analog node is still moving during the scan
(inductive ringdown, clamp recovery), the computed neutral is skewed by the scan order.

**Consequence for this milestone:** The display is useful for identifying gross timing
errors (ZC many samples off mid-sector). It is not suitable for sub-sample-precision
ZC timing without calibration.

### 3c. Instantaneous V_neut (stored in ring, used for display)

```rust
let s = (sector % 6) as usize;
let dma = core::ptr::addr_of!(ADC2_DMA_BUF).cast::<u16>();
let v_hi   = dma.add(PHASE_TO_DMA[HIGH[s]]).read_volatile();
let v_lo   = dma.add(PHASE_TO_DMA[LOW[s]]).read_volatile();
let v_neut = ((v_hi as u32 + v_lo as u32) >> 1) as u16;

RAW_NEUT_BUF[wr & MASK] = v_neut;
```

In async mode: write `0`. ZC analysis is suppressed entirely in async mode (§6f).

### 3d. IIR-filtered V_neut (future, for detection ISR only)

Use `rinz::ewma_pow2::EwmaPow2<K, T>` via `rinz::filter::FilterGeneric`. Do not
re-implement the algorithm.

```rust
use rinz::ewma_pow2::EwmaPow2;
use rinz::filter::FilterGeneric;

static FILT: EwmaPow2<4, i32> = EwmaPow2::new();
static mut STATE_HI: i32 = 0;
static mut STATE_LO: i32 = 0;

// Per TIM7 tick (triggered):
let v_neut_iir = ((FILT.filter(v_hi as i32, &mut STATE_HI)
                 + FILT.filter(v_lo as i32, &mut STATE_LO)) >> 1) as u16;

// Reset on sector entry:
STATE_HI = v_hi as i32;
STATE_LO = v_lo as i32;
```

K=4 → ~16 PWM periods ≈ 0.8 ms. **Not needed for the display milestone.**

---

## 4. Storage Changes

Add two parallel buffers. **Start with reduced sizes** to stay within SRAM budget;
verify with the linker map before increasing.

```rust
/// Instantaneous V_neut per TIM7 ring slot. 0 in async mode.
static mut RAW_NEUT_BUF:  [u16; RAW_SAMPLE_LEN] = [0u16; RAW_SAMPLE_LEN];
static mut SNAP_NEUT_BUF: [u16; SNAP_LEN]       = [0u16; SNAP_LEN];
```

**Recommended starting sizes (G431CB, 32 KB SRAM):**

| Buffer | Old size | New size | Bytes |
|--------|----------|----------|-------|
| RAW_SAMPLE_BUF | 4096 | **2048** | 4 KB |
| RAW_NEUT_BUF (new) | — | **2048** | 4 KB |
| SNAP_BUF | 2048 | 2048 | 4 KB |
| SNAP_NEUT_BUF (new) | — | **2048** | 4 KB |
| **Total** | 12 KB | **16 KB** | |

2048 samples = 21 ms at 96 kHz = 2.1 revs at 100 Hz floor. Adequate.

The previous plan of 4096 + 4096 + 2048 + 2048 = 24 KB left ~8 KB headroom, which
is insufficient once stack, HAL state, and formatted-print buffers are accounted for.
Use 2048 for RAW_SAMPLE_LEN. Bump to 4096 only after confirming linker map margin.

---

## 5. `e` Display: Phase Model and Semantics

**Explicit rule: `e` shows the `RING_CHAN_FIXED` phase as BEMF, same as `m`.**

`RAW_SAMPLE_BUF` stores `RING_CHAN_FIXED` (set by the `p` key: A/B/C). This is the
signal source for the BEMF row in `e`.

`RAW_NEUT_BUF` stores the sector-derived `(V_hi + V_lo)/2` independent of
`RING_CHAN_FIXED`. This is the signal source for the neut row in `e`.

Consequence: ZC analysis is meaningful only for sectors where `RING_CHAN_FIXED` is the
float phase (`role == fl`). For driven sectors (`hi`/`lo`), the neut row is omitted and
no ZC annotation is emitted.

This keeps `m`, `l`, and `e` consistent in their BEMF signal source. The user can
switch phase with `p` and re-run `e` to observe the other phases.

---

## 6. `e` Display Extension

### 6a. Snapshot copy

Copy both buffers under the same TIM7 mask in one block:

```rust
for i in 0..win_len {
    let src = (win_start as usize).wrapping_add(i) & RAW_SAMPLE_MASK;
    SNAP_BUF[i]      = RAW_SAMPLE_BUF[src];
    SNAP_NEUT_BUF[i] = RAW_NEUT_BUF[src];
}
```

### 6b. Per-sector output layout

Driven sectors: one row (unchanged from current `e`).

Float sectors: two rows, then ZC annotation in the sector header.

```
[s2 A=fl ZC@3/8]: 6a 43 44 46 *46 45 42 3d
[neut s2]:        35 35 35 35  35 35 35 35
```

Display format: 2 hex digits per value (v >> 4), space-separated.

### 6c. Row alignment — single-index loop (critical)

**BEMF row and neut row must correspond to the same ring indices.**

Do not independently de-staircase BEMF and neut. If they are compressed by separate
"print on change" loops, a BEMF column and its corresponding neut column will silently
refer to different ring slots whenever V_neut changes at a different sample than V_fl.
This makes the display look authoritative while the `*` marker is placed against a neut
value from a different time index.

**Correct approach:** single loop over `i` from `0` to `n`. Emit a column only when
`SNAP_BUF[rel + i]` differs from the previous emitted value. At that same `i`, also
read `SNAP_NEUT_BUF[rel + i]`. Accumulate both into parallel output vectors. Print
BEMF row, then neut row. Both rows have exactly the same number of columns, each
column pair from the same `i`.

```rust
let mut out_bemf = [0u16; SNAP_LEN];
let mut out_neut = [0u16; SNAP_LEN];
let mut col = 0usize;
let mut prev = u16::MAX;

for i in 0..n {
    let vb = SNAP_BUF[rel + i];
    if vb != prev {
        out_bemf[col] = vb;
        out_neut[col] = SNAP_NEUT_BUF[rel + i]; // same i — always aligned
        col += 1;
        prev = vb;
    }
}
// Print out_bemf[0..col] then out_neut[0..col]
```

### 6d. Blanking

```rust
const BLANK: usize = 2; // tunable; first guess from adc_dump_3/4 evidence
```

Skip the first `BLANK` samples before ZC scan. Both dump reviews confirm 1–2
contaminated transition samples at sector boundaries. `BLANK = 2` is the starting
value — not a settled constant. Speed-dependent and sector-dependent variations are
expected; treat it as a tuning parameter.

The column accumulation in §6c naturally includes blanked samples in the output (they
are displayed but not ZC-scanned). The ZC scan starts at column index corresponding to
ring index `BLANK`, not at column 0.

### 6e. Async mode guard

ZC scan and neut row are suppressed entirely when `ADC_TRIGGERED` is false.

```rust
let triggered = ADC_TRIGGERED.load(Ordering::Relaxed);
// For float sectors:
if triggered {
    // print neut row, run ZC scan, emit ZC annotation
} else {
    // print BEMF row only; no neut row, no * marker, no ZC annotation
}
```

When `RAW_NEUT_BUF` is all zeros (async mode), ZC detection would produce garbage:
descending ramps immediately read `ZC<0` (already crossed 0), ascending ramps read
`ZC>n` (never reach 0). Both outcomes are meaningless artifacts.

### 6f. ZC direction — from table, not from data

**Never infer ramp direction from the post-blank sample value.**

At higher speeds (confirmed at ≥280 Hz open-loop), the ZC has already occurred before
the sector starts. The first post-blank sample is then already on the far side of
neutral, causing data-driven direction inference to flip to the wrong polarity.

Direction is derived from the six-step table: the float phase has a descending ramp if
it was HIGH in the previous sector, and an ascending ramp if it was LOW.

```rust
// Precompute: ramp_falling[s] = was float_phase HIGH in sector (s+5)%6?
const RAMP_FALLING: [bool; 6] = {
    let mut t = [false; 6];
    let mut s = 0usize;
    while s < 6 {
        let fp = 3 - HIGH[s] - LOW[s];
        let ps = (s + 5) % 6;
        t[s] = HIGH[ps] == fp;
        s += 1;
    }
    t
};
// Phase A (fp=0): s2 → true (falling, A was hi in s1), s5 → false (rising, A was lo in s4)
```

### 6g. ZC classification — three states with consecutive confirmation

One threshold comparison is insufficient to classify ZC state reliably. A contaminated
post-blank sample, a noisy float near neutral, or a biased V_neut estimate can all
trigger a false classification. Require **2 consecutive samples** on the crossing side.

```rust
const BLANK: usize = 2;
const ZC_CONFIRM: usize = 2; // consecutive samples required to confirm crossing

enum ZcResult { Late, At(usize), Early }

let ramp_falling = RAMP_FALLING[sec];

fn crossed(vf: u16, vn: u16, falling: bool) -> bool {
    if falling { vf <= vn } else { vf >= vn }
}

let zc = if n > BLANK + ZC_CONFIRM {
    // Check if already across neutral at first post-blank sample (ZC before sector start)
    let confirmed_late = (BLANK..(BLANK + ZC_CONFIRM)).all(|i| {
        crossed(SNAP_BUF[rel + i], SNAP_NEUT_BUF[rel + i], ramp_falling)
    });

    if confirmed_late {
        ZcResult::Late
    } else {
        let mut found = None;
        'outer: for i in (BLANK + 1)..(n.saturating_sub(ZC_CONFIRM - 1)) {
            let all_crossed = (i..(i + ZC_CONFIRM)).all(|j| {
                crossed(SNAP_BUF[rel + j], SNAP_NEUT_BUF[rel + j], ramp_falling)
            });
            if all_crossed {
                found = Some(i);
                break 'outer;
            }
        }
        match found {
            Some(i) => ZcResult::At(i),
            None    => ZcResult::Early,
        }
    }
} else {
    ZcResult::Early
};
```

Emit `*` in the BEMF row before the column whose ring index is the confirmed crossing
sample (`ZcResult::At(i)`). For `ZcResult::Late` and `ZcResult::Early`, emit the
annotation in the sector header only — no `*` in data rows.

---

## 7. ZC Timing Sign Convention

**One canonical definition. Do not deviate.**

```
Commutation_Advance_Error = zc_idx − (M / 2)
```

where `M` = total sector sample count, `zc_idx` is the ring-slot offset of the
confirmed crossing (conceptually negative for `ZC<0`).

| ZC position | Error sign | Stator is... | Action required |
|-------------|-----------|-------------|-----------------|
| ZC < 0 (before sector) | Highly negative | **LATE / retarded** | Advance phase |
| 0 ≤ ZC < M/2 | Negative | **LATE / retarded** | Advance phase |
| ZC = M/2 | Zero | **IDEAL** | None |
| M/2 < ZC ≤ M | Positive | **EARLY / advanced** | Retard phase |
| ZC > M (after sector) | Highly positive | **EARLY / advanced** | Retard phase |

**Memory aid:** negative error → stator lags rotor → advance to catch up.

`ZC<0` is the dominant case at higher open-loop speeds (motor spinning faster than
commanded electrical phase). The display tool's primary purpose is to measure how far
negative the error is across the operating range.

Physical interpretation: the ideal ZC point is at sector mid-point (30° into a 60°
window = 90° from the preceding commutation edge). A ZC arriving early in the sector
(small index) means the rotor is ahead of the stator. A ZC arriving late (large index
or after the sector) means the rotor is behind.

---

## 8. What Is NOT Changed

- Ring write rate (96 kHz), sector-start bookmarking, ring reset logic.
- `m` and `l` behavior (only display format changes, §1).
- `configure_adc_capture`, `ADC_TRIGGERED`, `RING_CHAN_FIXED`, `p` key.
- TIM7 ISR structure — neut write is added alongside existing BEMF write, not replacing it.
- Async mode operation — ring and display continue to work; ZC analysis is suppressed.

---

## 9. Implementation Steps

1. **Reduce `RAW_SAMPLE_LEN` to 2048.** Verify linker map; only raise if headroom confirmed.

2. **Add `RAW_NEUT_BUF` and `SNAP_NEUT_BUF` statics.** Build (neut all-zero until step 3).

3. **TIM7 neut write.** In triggered branch: compute instantaneous V_neut from DMA buf
   using sector-derived hi/lo; write to `RAW_NEUT_BUF[wr]` alongside existing BEMF write.

4. **`e` snapshot.** Add `SNAP_NEUT_BUF` copy inside the existing TIM7-masked block.

5. **`e` print loop.** For float sectors (triggered mode only): run the single-index
   column accumulation (§6c), print BEMF row then neut row, both with 2-char shifted hex.

6. **ZC detection.** Add `RAMP_FALLING` table, blanking, consecutive-confirmation scan
   (§6f/6g). Emit `*` and `ZC@N/M` / `ZC<0` / `ZC>n` annotations.

7. **Validate.** See §10.

---

## 10. Validation Criteria

| Check | Pass condition |
|-------|---------------|
| Memory | Linker succeeds; map shows ≥4 KB SRAM headroom after all statics + stack |
| Row alignment | BEMF and neut rows always have equal column count; no column-count mismatch |
| Blanking | No `*` on columns from ring indices 0 or 1 |
| Driven sectors | No neut row, no `*`, no `ZC` annotation on `hi`/`lo` sectors |
| Async guard | No `ZC` annotation or `*` when ADC is in async-legacy mode |
| V_neut stability | Neut row values vary by ≤ 4 LSB (shifted) within a sector at steady speed |
| In-sector ZC at low speed | At ≤ 200 Hz: `ZC@N/M` with N in `[BLANK, M−1]` in both float sectors |
| Late ZC at mid speed | At ~280 Hz open-loop: `ZC<0` in float sectors (confirmed by adc_dump_4) |
| High-speed graceful | At ≥ 400 Hz: `ZC<0` or `ZC>n` reported; no crash; no false `*` on plateau |
| Sign sanity | At any speed where `ZC<0`: error is negative → "stator late → advance" |

---

## 11. Open Questions (Future Work)

- **V_neut calibration offset:** If systematic bias is confirmed in hardware, a per-sector
  trim constant may be needed. Not addressable until ZC display is running and comparable
  to a reference.
- **Scan-order correction:** Compensating for the sequential-not-simultaneous ADC scan
  would require knowing the rate of change of each phase during the scan window. Deferred.
- **IIR neut for ISR path:** `EwmaPow2<4, i32>` as described in §3d. Only after display
  milestone confirms the instantaneous neut is useful.
- **Multi-revolution averaging of ZC offset:** Reduces per-rev noise in the error signal.
  Not needed for display; needed before any closed-loop controller is written.
- **`BLANK` auto-tuning:** Could be derived from the measured inductive decay rate at each
  speed. Deferred; manual tuning sufficient for the display milestone.
