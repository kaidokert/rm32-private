No definite containment bug is demonstrated by this excerpt. `off_before`/`gates_before` now prevent cleanup from manufacturing the final disabled readback, but remain point samples: they do not establish continuous gate containment.

Case 4 tests whether a COM pending inside PRIMASK is cleared before unmasking; its replacement ISR only counts/stops, so it cannot validate production COM behavior. That is consistent with the stated narrow scope.

Case 3’s bounded delay does not guarantee 20 µs elapsed; insufficient delay should fail the expected-reason assertion, not qualify the case.

Remaining qualification blocker: verified flashed artifact and actual five-case results, including pre-cleanup gate readbacks and Vref stability. Release/clippy/audit success supplies neither. Priority readback improves evidence, but does not prove interrupt delivery or timing.
