Yes—bounded experimental integration is defensible under the stated invariants, conditional on the planned tests and audits. Five observation slots differ materially from foreground polling; bounded servicing alone establishes neither recovery nor a cure.

Concrete issue: **the existing admission helper rejects phase 4.** `sector_ready()` requires `com_phase == 0`, while the design runs rechecks in phase 4 and requires the “full old live gate.” As written, every phase-4 observation using that helper is refused, so the experiment could produce no retries despite valid comparator levels. Allowing 0/4 only in *refusal resume* does not fix admission.

Define an explicit candidate admission rule before coding: phase 4 qualifies only with current generation, listening authority, and active/non-stopped state verified under the same exclusion as reservation and pend. Preserve rejection of phases 1/2/3; avoid temporarily clearing phase merely to pass the old helper.

Require transition tests showing a valid phase-4 observation can pend, while acceptance, stop, or generation replacement between scheduling and service prevents both the pend and any obsolete timer rearm.
