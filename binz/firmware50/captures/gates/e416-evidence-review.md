Raw: 4 tests passed, 372 filtered out. Incremental-cache access warning did not prevent completion. Each immediate-write test covers 12 directed adjacent transitions × 2 PWM levels × 3 writes = 72 snapshots, including final states. Reported counts: baseline 10 unintended-low channel observations and 2 dual-high snapshots; diode variant 0 and 2. These are observations, not distinct transitions or pulses.

Latch coverage: 36 ordered sector pairs, including self-transitions, × 2 levels = 72 cases. Diode transform: 6 sectors × 4 duties = 24 plans, checked at 48 level combinations.

Proved within the model: source NE alone changes; immediate writes retain dual-high intermediate states. Zero diode unintended lows is printed, not asserted. Same-leg exclusion follows structurally from the model.

Atomic latching is assumed by whole-structure assignment; hardware preload semantics, interruption safety, pulse timing, currents, and fault causation remain unproved. Reference code supports selectable source-low disabling, not transition equivalence. Proposed interpretation is appropriately bounded; unchanged production/bench status requires separate evidence.
