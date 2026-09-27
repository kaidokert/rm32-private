Raw assembly confirms repeated volatile loads at `0x8000912`, per-read `SUBS/SBCS` normalization and `NEGS/ADCS` polarity conversion, followed by mismatch rejection and the depth-controlled backedge. Removing those conversions plausibly shortens the sampling aperture.

Proceed offline. No concrete semantic flaw is evident, provided:

- `level_word()` returns only the comparator output mask, from the same register through a volatile read.
- `expectedword` is exactly zero or that mask, with polarity matching `edge_is_rising(step)`.
- The predicate implementation preserves read count, order, early rejection, state updates, and timing-guard placement. The wrapper must invoke `read_level()` once per tested sample.
- The logged root remains unchanged.

Exhaustive Boolean-stream tests can establish unchanged digital decisions for identical supplied sequences. They cannot establish identical physical behavior: faster reads sample different instants and can accept shorter noise pulses despite unchanged persistence counts. Assembly verification must cover both paths.

The supplied stop log supports only the recorded 25% propless run; it does not validate 80% operation or the modified aperture. Offline approval grants no authority to relax existing protections.
