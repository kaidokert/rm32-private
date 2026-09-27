The correction closes the visible software interleaving window: authority sampling, observation, quota reservation, pend, and timer arm share interrupt exclusion. Guard no longer borrows the model, and reporting checks inactive owners and disabled outputs.

Two obligations remain:

- COM can enter after preempting an already-running COMP publication. Masking then prevents further execution; it does not complete the interrupted publication. Safety depends on generation invalidation preceding every relevant field mutation and on timer/IRQ cleanup. Those paths are absent, so the comment alone cannot establish this.
- Reconstructing origin from a 16-bit acceptance timestamp aliases ages separated by 65,536 µs. The extended-clock cutoff cannot detect that alias when creating a schedule. The author appropriately makes correctness conditional on existing tracking bounds.

The raw run demonstrates a stale trip at 243 µs against 240 µs, zero hold time, and disabled outputs afterward. It contains no `TIMEDRECHECK` row and does not validate this correction’s masked latency.

Verdict: plausible scoped correction; publication ordering and measured interrupt latency remain unverified here.
