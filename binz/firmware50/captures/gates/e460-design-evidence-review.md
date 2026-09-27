No reachable counterexample **provided `crate::revisit::AVERAGE_MIN_US == 32`**. That constant’s definition is absent, so the hardcoded lower bound cannot be verified from this excerpt.

For valid averages, let `D(k)` be the shared deadline formula. It strictly increases, including for odd averages, because both implementations use `average >> 1`.

Initially `used = slot = 0`. At any due observation, `used <= slot`, so `D(used) <= D(slot) <= elapsed`. Consequently, with ownership and elapsed checks passed, `reserve` returns exactly `live_admitted`.

The advancement formulas select the first slot whose deadline exceeds elapsed, or terminal slot 5. Thus they strictly advance the slot; whether admission succeeds or fails, `used <= slot` remains true. Rejected observations consume a slot without breaking equivalence. Late observations skip identically.

Budget exhaustion cannot occur with a remaining slot: `used == 5` implies `slot == 5`. Cancellation and invalid estimates are equivalently terminal.

The supplied hardware trace does not establish this equivalence; the invariant does. If the imported minimum differs from 32, constructor behavior can diverge.
