# BEMF Raw Capture Design

## Goal

Replace the current heuristic `l` dump with an exact dump of the last two full electrical revolutions, aligned to commanded commutation boundaries.

The dump must:

- use commanded sector changes only
- start exactly at commanded `sector 0` output commutation
- cover exactly two full completed revolutions
- require storage for three revolution starts
- sample raw BEMF ADC at full `96 kHz`
- be disabled unless `electrical_hz > 100`

This design is for raw ADC capture, not zero-cross reconstruction.

## Requirements

- Sample source is the existing `read_bemf_raw_pac(...)` path.
- Sector source is the commanded sector from `sector = (STEP / 8) as u8`.
- No backward scanning of the sample ring to guess where revolutions started.
- No use of ADC data to infer sector.
- `l` must refuse to print unless there are three captured commanded `sector 0` boundaries.
- A complete dump is only valid when `electrical_hz > 100`.

## Non-Goals

- No zero-cross detection in this path.
- No thresholding or edge extraction.
- No reconstruction of missing sector labels from data.
- No support for very low RPM. If there is not enough RAM/time budget below `100 Hz`, the feature stays disabled.

## High-Level Approach

Use one raw sample ring at `96 kHz` and a separate small boundary history written by the TIM7 ISR.

At every TIM7 tick in six-step mode:

1. Determine commanded sector from the PWM/commutation state.
2. Sample the floating phase raw ADC with `read_bemf_raw_pac(...)`.
3. Write the raw `u16` sample into a ring buffer.
4. Increment the sample write index.
5. On commanded sector change, record the current sample index as the start of that sector.
6. On commanded `5 -> 0`, record the current sample index as a revolution start.

The `l` command does not search the sample ring. It snapshots the sample ring plus the last three completed revolution starts and their sector boundaries, then prints exactly:

- rev 0 sectors 0..5
- rev 1 sectors 0..5

The window is:

- `start = rev_start[oldest_of_last_3]`
- `end = rev_start[newest_of_last_3]`

That is exactly two full completed revolutions.

## Data Model

### Raw Sample Ring

Store raw ADC only:

```rust
const RAW_SAMPLE_LEN: usize = 4096;
static RAW_SAMPLE_BUF: [u16; RAW_SAMPLE_LEN] = [0; RAW_SAMPLE_LEN];
static RAW_SAMPLE_WR: AtomicU32 = AtomicU32::new(0);
```

Notes:

- `4096` is a power of two, so wrapping is one mask.
- `u16` preserves full ADC resolution.
- Sector is not stored per sample; boundaries define the slicing.

### Revolution History

Keep the last three revolution starts in sample-index units.

Recommended shape:

```rust
use heapless::HistoryBuffer;

static mut REV_STARTS: HistoryBuffer<u32, 3> = HistoryBuffer::new();
```

Each entry is the sample write index at the moment commanded `sector 0` starts.

Alternative if avoiding `heapless` here:

- fixed `[u32; 3]`
- head/count bookkeeping

Either is acceptable. `HistoryBuffer` is fine here because:

- only TIM7 writes
- main reads after masking TIM7
- capacity is tiny

### Sector Boundary History

For each of the last three revolutions, keep six sector start indices.

Recommended shape:

```rust
static mut REV_SECTOR_STARTS: HistoryBuffer<[u32; 6], 3> = HistoryBuffer::new();
static mut CUR_REV_SECTOR_STARTS: [u32; 6] = [0; 6];
```

Behavior:

- on each sector change, write current sample index into `CUR_REV_SECTOR_STARTS[sector]`
- on commanded `5 -> 0`:
  - push completed `CUR_REV_SECTOR_STARTS` into `REV_SECTOR_STARTS`
  - push current sample index into `REV_STARTS`
  - start a fresh `CUR_REV_SECTOR_STARTS`
  - immediately set slot `0` for the new revolution

Important invariant:

- `REV_STARTS[i] == REV_SECTOR_STARTS[i][0]`

## Enable Rule

Raw capture is enabled only when all of these hold:

- `RUNNING == true`
- mode is six-step
- `electrical_hz > 100`

If `electrical_hz <= 100`:

- do not push raw samples
- do not update revolution history
- `l` prints a clear refusal message

Example:

```text
l: disabled below 100 Hz (current 87 Hz)
```

This avoids pretending the dump is valid when the retention window is too short.

## Memory Budget

Assume full-rate `96 kHz` capture and a minimum valid electrical frequency of `101 Hz`.

Three electrical revolutions take:

- `3 / 101 s = 29.70 ms`

Samples required at `96 kHz`:

- `96_000 * 0.02970 = 2851.2`

So a `4096`-sample ring covers comfortably more than three revolutions at the allowed minimum frequency.

### RAM Cost

Raw sample ring:

- `4096 * 2 bytes = 8192 bytes`

Boundary history:

- `REV_STARTS`: `3 * 4 = 12 bytes`
- `REV_SECTOR_STARTS`: `3 * 6 * 4 = 72 bytes`
- `CUR_REV_SECTOR_STARTS`: `6 * 4 = 24 bytes`

Total main payload:

- about `8300 bytes`

That fits in `32K` RAM even with the rest of the example state.

### Why Not Smaller?

At `96 kHz`, a `2048`-sample ring only covers:

- `2048 / 96_000 = 21.33 ms`

Three revolutions at `100 Hz` need `30 ms`, so `2048` is not enough.

### Why Not Downsample?

Not needed for the stated requirement.

Full-rate `96 kHz` raw capture is affordable in RAM if the feature is disabled below `100 Hz`.

## ISR Behavior

TIM7 remains the sole writer of capture state.

Per tick:

1. If capture disabled, return from raw-capture path.
2. Compute commanded sector.
3. Read raw ADC for the commanded floating phase.
4. Store raw sample at `RAW_SAMPLE_WR & (RAW_SAMPLE_LEN - 1)`.
5. Increment `RAW_SAMPLE_WR`.
6. If sector changed:
   - record current sample index in `CUR_REV_SECTOR_STARTS[sector]`
7. If commanded transition was `5 -> 0`:
   - finalize previous revolution
   - push completed `CUR_REV_SECTOR_STARTS` into history
   - push current sample index into `REV_STARTS`
   - clear/init next `CUR_REV_SECTOR_STARTS`
   - set new revolution slot `0`

Notes:

- Use sample index, not time tick, for all dump boundaries.
- This makes slicing exact in the same domain as the stored samples.
- No conversion from time to sample offset is needed.

## `l` Command Behavior

The `l` command should:

1. Mask TIM7.
2. Snapshot:
   - raw sample ring
   - current write index
   - last three revolution starts
   - last three `[u32; 6]` sector-start arrays
3. Unmask TIM7.
4. Validate:
   - capture enabled
   - exactly three stored revolution starts available
   - indices are monotonic in ring-index space
5. Print exactly two full revolutions:
   - `r0 s0..s5`
   - `r1 s0..s5`

Formatting should not be based on observed sector changes in the sample data.

Instead, print fixed ranges:

- `r0 s0 = starts[0][0] .. starts[0][1]`
- `r0 s1 = starts[0][1] .. starts[0][2]`
- ...
- `r0 s5 = starts[0][5] .. starts[1][0]`
- `r1 s0 = starts[1][0] .. starts[1][1]`
- ...
- `r1 s5 = starts[1][5] .. starts[2][0]`

Where:

- `starts[0]`, `starts[1]`, `starts[2]` are the oldest-to-newest entries from the snapshot history

This guarantees:

- exactly 12 rows
- no duplicate sector labels
- no missing sectors
- exact alignment to commanded sector starts

## Dump Resolution

The raw store should remain full `u16`.

The printed output can be downsampled for UART readability.

Recommended dump formatting:

- keep storage full-rate/full-resolution
- downsample only at print time
- choose a target width per sector row, for example `48` or `64` columns

For each printed column:

- map a sample span to one glyph
- choose one representative value from the span:
  - max
  - min
  - average
  - first sample

Recommended first implementation:

- use max or average per bin
- map 12-bit ADC to one hex digit or one ASCII ramp character

The storage format should not be compromised just to make UART output shorter.

## State Reset Rules

Reset capture history when:

- motor is killed
- mode changes away from six-step
- `electrical_hz` crosses from `<= 100` to `> 100`
- user issues reset command

This avoids mixing partial old windows with a new run.

## Invariants

These should always be true:

- sector history comes only from commanded commutation
- sample ring indices are the only coordinates used by `l`
- revolution start is exactly the sample index when commanded sector changes to `0`
- last two full revolutions means `rev_start[n-2] .. rev_start[n]`
- `l` never guesses revolutions from sample contents

## Implementation Steps

1. Add `heapless` dependency if using `HistoryBuffer`.
2. Replace the current `PWM_SAMPLE_BUF: [AtomicU8; N]` path for `l` with a `u16` raw sample ring.
3. Add revolution-start and sector-start history in sample-index units.
4. Update TIM7 raw-capture path to write:
   - sample
   - sector boundary indices
   - revolution starts
5. Gate the entire feature on `electrical_hz > 100`.
6. Rewrite `l` to print from exact stored revolution/sector boundaries.
7. Remove backward scan logic and any formatting based on sector nibble changes in sample data.

## Expected Outcome

After this change:

- `l` becomes deterministic
- every dump shows exactly two full completed revolutions
- every row boundary matches commanded sector transitions exactly
- no sector labels are repeated or skipped
- raw ADC is preserved at full `96 kHz` and `u16` resolution
