Raw decoding:

- Flash log reports successful download, verification, and reset for SHA `CA94ACF6…ADD8D`.
- Case 0: `before=18`, `after=19`, `com_calls=1`, `masked=1`, `off=1`, `pass=0`.
- Snapshot 18 means active + off; 19 additionally means stopped. Both show timer-running and TIM16-pending bits clear.
- Address calculation matches: `0x20000190 + 0x4c = 0x200001dc`. Readback decodes to avg/last/previous=1000, too_early=0, unstable=1, accepted=0, bounds=1..4000.
- Disassembly increments `unstable` on comparator-level mismatch during the persistence loop.

Interpretation: assuming the readback belongs to this case without intervening detector changes, it supports persistence-filter refusal before acceptance/arming. It does **not** demonstrate a masking failure or establish an analog cause. `masked=1` samples PRIMASK at one point. The refusal-wake → probe-COM-stop explanation fits the observations; the omitted resume predicate prevents fully proving that chain here.

The proposed disabled-only fixture change is reasonable: select negative input 3, delay, then sample the expected level. The supplied HAL identifies 3 as full VREFINT; it does not verify `select_negative` implementation or voltage separation. Keep the change within the off-only preparation path and rerun the suite. Stability and acceptance remain unproven. No motor authorization.
