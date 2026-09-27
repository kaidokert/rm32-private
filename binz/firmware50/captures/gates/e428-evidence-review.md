The raw trace supports an incomplete ramp, not successful target operation: `hold_ms=0`; reported CCR293/1333 is **21.98%**, below the 25% target. This register ratio does not independently measure delivered phase voltage.

Ordinal joining yields 127 pairs, one wide bracket (25 ticks = 3.125µs), and maximum entry-to-postapply delay minus wait of 8µs.

Terminal chronology:
- 371696: entry24040, interval32, average51, wait13; postapply24059—19µs after entry, 6µs beyond wait.
- 371697: entry24084, interval44, average44, wait11; no matching commutation. Its entry follows the previous postapply by 25µs.

`spent_at_late=11` equals that terminal wait, consistent with exhausted scheduling budget. However, the aggregate late-arm report lacks an ordinal; attribution to 371697 and decoding reason28 require implementation evidence.

FreshEstimate’s causal responsibility is unproven. Frozen-input tests establish arithmetic tradeoffs, not hardware survival. E425’s claimed exposure is not independently reviewable here.

Repeating both now offers little rate information without matched exposure and sufficient trials. Prioritize bounded offline cost reduction, preserving timing semantics and guards; verify generated-code savings. Keep fresh unqualified, but do not infer intrinsic inferiority or a fix for tracking/edge identity.
