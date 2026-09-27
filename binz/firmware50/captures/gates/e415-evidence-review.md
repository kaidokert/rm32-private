Recomputed from raw:

- Target hold: **3.662 s**, versus 23.662 s closed operation.
- Tail rate: 54,849 / 3.662169 = **14,977.2 accepts/s**, or **2,496.2 electrical Hz**, assuming six accepts/cycle. Rounded 66 µs gives 2,525.3 Hz; these are different estimators.
- The 32 coast intervals average **206.719 µs**, implying **2,418.7 eHz** if each interval represents half an electrical cycle.
- Raw bus minimum: 1101/1211 = **90.92%**, a **9.08% drop**. Minimum-time VREF is missing, so this is not a normalized voltage-drop measurement.
- Filtered bus/VREF ratio relative to reference is **100.099%**; that does not negate the recorded three-scan sag trip.
- Sector accepts sum to **230,873**, exactly **2,374** below total accepts, matching mailbox coalescing.
- Applied compare fraction: 666/1333 = **49.962%**.

Unsupported conclusions include calibrated current, adequate current headroom, absence of preemption or timing-margin violations from zero counters, trustworthy unsaturated phase distributions, and a demonstrated physical cause of sag. `NoLog`/`NoChain` and zero-valued diagnostics require instrumentation verification.

The source transfers roles at COMG but compares at the next native update; it does **not** establish simultaneous role-and-duty transfer or harmless intermediate behavior.

`System.Object[]` supplies no substantive author interpretation.

**Higher duty is not admitted.** This single 50% screen failed with reason 26 after only 3.662 s at target. Post-stop safing passed; sustained operation did not.
