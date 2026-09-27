Bounded experimental integration is defensible, but the design has a concrete admission mismatch to resolve before coding.

`sector_ready()` requires `com_phase == 0`. Timer rechecks execute as phase 4, yet the design requires every request to pass the full old live gate. Consequently, a phase-4 callback using that predicate refuses every request. Allowing refusal resume in phases 0/4 does not fix admission.

Specify one consistent transition: either retire phase 4 to phase 0 under exclusion before admission, after validating generation and timer ownership, or introduce a candidate-specific predicate that authorizes phase 4 only with those validations. Preserve phase-1/2/3 exclusion.

Require transition tests showing that a valid phase-4 observation can actually pend COMP, while an intervening acceptance or stop prevents both pending COMP and rearming TIM16.

Five observation slots explicitly differ from foreground polling. The quota and skipped expired slots bound activity; they do not establish that observations coincide with an admissible comparator level or cure missed crossings.
