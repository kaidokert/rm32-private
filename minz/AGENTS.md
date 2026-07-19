# Agent Notes

## 2026-05-19 comparator polarity sweep (`examples/motor_tester.rs`)

This repo already has broad comparator/background notes in
[`CLAUDE.md`](CLAUDE.md), [`COMP2_DIAGNOSIS.md`](COMP2_DIAGNOSIS.md),
and [`COMP_SAMPLING.md`](COMP_SAMPLING.md). This note captures the more
specific read of the manual dump where the bench user swept COMP2
polarity modes while observing phase `A`.

### Relevant dump semantics

- `obs=A` means the observed phase is textbook phase A, which floats in
  sectors `2` and `5`.
- In the `pwm_samples` dump, `#` means `comp2::value()==1`, `.` means
  `0`.
- In float sectors, the midpoint marker is overlaid on the sample:
  `*` means the midpoint sample was `#`, `o` means it was `.`.
- The `edges` dump is coarser: each digit is how many 10 us bins inside
  an 80 us chunk saw at least one COMP EXTI edge.

### What the dump says

- `always +` (`POLARITY=0`) is not giving a clean A-phase zero-crossing
  picture. Sector `2` is usually high almost the whole time
  (`####*###`, `#..#*###`), while sector `5` is usually low almost the
  whole time (`....o...`). That is not the expected "transition near the
  middle of both float sectors" pattern.
- `+sec0-2/-sec3-5` is the best of the tested modes. In sector `2`, the
  pre-midpoint half is mixed while the post-midpoint half is usually
  high (`#.##*###`, `#...*###`, `##.#*###`), which at least looks like a
  usable late-sector settle. Non-float sectors also become more
  symmetric between revs.
- Even in that better mode, sector `5` still tends to rail high after
  inversion (`####*###`) rather than showing a crisp midpoint crossing.
  So polarity switching helps, but does not by itself produce a clean
  "one crossing per float sector" waveform.
- `-sec0-2/+sec3-5` is clearly worse. The float sectors collapse toward
  mostly-low patterns (`....o...`, `.##.o...`, `..#.o...`) and rev to
  rev stability degrades.
- The `edges` dumps stay dense in nearly every sector under both mixed
  polarity modes, typically `2..5` occupied 10 us bins per 80 us chunk.
  That means polarity changes alone are not isolating a single BEMF
  crossing; PWM-coupled activity is still spread across the revolution.

### Practical conclusion

If revisiting this line of work, treat `+sec0-2/-sec3-5` as the current
best empirical polarity mode for `obs=A`, but only as a relative win.
It improves sector-2 shape; it does **not** fully solve sector-5 or the
overall edge density problem.

### 2026-05-19 follow-up: high-resolution edge dump at `f=490`

Later data at `f=490`, `rev=2040 us`, `polarity=-sec0-2/+sec3-5`,
`hyst=3` used the cleaned-up high-resolution edge formatter. That dump
adds one useful nuance:

- The edge field is no longer uniformly dense. Sectors `0`, `1`, and
  `3` still contain repeated edge trains across most of the sector.
- Sector `4` becomes sparse and often collapses toward a short early
  burst followed by a mostly quiet tail.
- Sector `5` is the quietest region by far, often nearly empty except
  for a few late stragglers or a short cluster near one edge.

So the comparator activity is becoming strongly sector-structured rather
than "PWM noise everywhere". That is progress. But for `obs=A` it still
does not place the quiet/active split where a clean textbook A-phase
zero-crossing detector would want it:

- A should float in sectors `2` and `5`.
- The new dump makes `5` look special, but `2` is still busy almost all
  the way through.
- That asymmetry still points to polarity/expected-level semantics or
  phase interpretation being incomplete, rather than this mode being
  fully correct.

### 2026-05-19 phase sweep with `edges=rise`, `polarity=always +`, `hyst=3`

This later sweep is more diagnostic than the earlier mixed-edge dumps.
With rising edges only, each observed phase shows one clearly quieter
sector that matches the textbook **rising-ZC** float sector:

- observe phase `A` -> quietest sector is `5`
- observe phase `B` -> quietest sector is `1`
- observe phase `C` -> quietest sector is `3`

That matches the known six-step convention already used in code:

- phase `A` floats in `2` and `5`; of those, `5` is the rising-ZC one
- phase `B` floats in `1` and `4`; of those, `1` is the rising-ZC one
- phase `C` floats in `0` and `3`; of those, `3` is the rising-ZC one

So this sweep is strong evidence that:

- the observed-phase mapping `A->(2,5)`, `B->(1,4)`, `C->(0,3)` is
  fundamentally correct
- the sector numbering is coherent
- selecting only rising edges reveals the expected once-per-rev
  "special" float window for each phase much more clearly than the
  earlier both-edge view

What it does **not** prove is that the full polarity/filter pipeline is
finished. The non-rising float sector for each phase still needs the
complementary falling-edge interpretation, and any polarity-switched
mode still needs `EXPECTED_POST_ZC` semantics checked against the active
comparator polarity.

### Important code caveat

`TIM7` currently derives `EXPECTED_POST_ZC` from sector parity alone:

- even sector -> `true`
- odd sector -> `false`

That matches non-inverted comparator semantics only. Once
`comp2::set_polarity()` is used in modes `1`, `2`, or `3`,
`comp2::value()` changes meaning, but `EXPECTED_POST_ZC` is not adjusted
to match. Any `valid` / `filt` interpretation under inverted or
sector-swapped polarity therefore needs caution until the expected-level
logic is made polarity-aware.
