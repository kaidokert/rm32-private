RAW recomputation: acceptance gaps are 474, 442, 339, 792, 506, 1035 µs; sectors advance 1→2→3→4→5→6→1. For IDs 289144–289149, application minus crossing minus requested wait is 5, 6, 6, 5, 6, 5 µs.

The terminal acceptance at 21655 follows 20620 by 1035 µs, exceeding the reported 1000-µs ceiling. `event()` polls before updating `last`, so this gap alone suffices to latch Stale **if those timestamps match the watch’s timebase**; `guard_event` is absent here. Detector acceptance publishes and arms before calling the guard. Thus a terminal acceptance record without matching COM is consistent with shutdown following acceptance, not proof of a stalled commutator.

Proceed offline with the separate NoChain binary as a falsifiable overhead experiment:

- Verify emitted roots actually remove diagnostic work while retaining detector, guard, advance, and bridge behavior.
- Check deadline arithmetic and arm-before-guard ordering remain intact.
- Quantify instruction/cycle changes; smaller code alone does not establish lower execution cost.
- Make no startup-success or 80%-duty claim from this trace.

Existing diagnostics do not establish overhead as the cause.
