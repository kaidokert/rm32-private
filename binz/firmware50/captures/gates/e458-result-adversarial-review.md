The failure is real; its cause is unresolved. Case 1 recorded one observation, zero requests, phase 4 and timer running. That does not identify which admission input refused, or exclude a probe/timebase defect. Cases 2–5 provide no evidence because they did not run.

72/81 ticks convert to 9/10.125 µs, assuming the stated tick scale. They measure the bracketed `after_phase` calls, excluding interrupt entry, preceding setup and subsequent cleanup. Neither is WCET; the failed case does not establish successful-pend timing or transfer to powered operation.

Next action: instrument the off-only probe with a bounded memory snapshot of the **actual decision inputs**: authority/ownership, raw/base/extended clock and origin, elapsed/due, line-live, pending, comparator polarity match, budget state and decision. Report after shutdown; avoid diagnostic resampling as evidence of earlier conditions. Check the independently constructed schedule origin against clock reconstruction.

Retain the failing expectation. Sampled off checks support those checkpoints, not continuous-off proof. **No motor admission.**
