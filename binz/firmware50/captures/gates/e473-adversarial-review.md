Raw decoding:

- Flash/download/verify/reset report success for image `CA94ACF6…ADD8D`.
- Only case 0 is captured: `pass=0`, `reason=0`, `masked=1`, `off=1`, `com_calls=1`.
- `before=18`: active, off; stopped/running/pending clear. `after=19`: additionally stopped; active remains set.
- RAM address arithmetic is correct: `0x20000190 + 0x4c = 0x200001dc`. Decoded fields are avg/last/previous=1000, too_early=0, unstable=1, accepted=0, bounds=1..4000.
- Disassembly increments unstable on comparator-level mismatch during filtering.

Interpretation: assuming readback belongs to this run without intervening mutation, the evidence supports persistence-filter refusal before arming. The COM count and snapshots are consistent with refusal waking TIM16 and the probe handler stopping COM. The supplied excerpts do not independently prove that entire causal chain. This is failed acceptance coverage, not demonstrated masking failure; no analog cause is established.

The proposed disabled-fixture change is reasonable conditionally: place selection and settling before expected-level sampling, verify `select_negative(3)` actually programs INMSEL, and confirm this fixture remains exclusive to the disabled binary. HAL supports encoding 3 as full VREFINT; it does not establish voltage separation or sufficient settling.

Rerun all cases with outputs disabled. No motor authorization.
