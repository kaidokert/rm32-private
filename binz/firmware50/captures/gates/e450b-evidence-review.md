The revised deferred path closes the shown bypass: guard/driven checks run inside the existing interrupt mask; enabling goes through the shared policy; software pending occurs only after successful resume. Detector ownership takes precedence, preventing driven activity from overriding closed-loop blanking or stop.

Ownership/caller evidence supports the shown startup paths: `drv_begin` publishes driven activity before arming, and `driven_boundary` arms during acquisition. These excerpts do not establish an exhaustive caller inventory.

Concrete remaining defects: none demonstrated in the supplied evidence.

Unproven general properties: repository-wide bypass freedom, hardware timing, and complete ownership-transition safety. The policy test checks the truth table; the race model assumes shutdown masks the line; source-string tests check textual ordering, not execution semantics. The supplied log reports 390 passing tests despite an incremental-cache warning. ISR identity needs its comparison baseline identified before it supports this revision. Candidate remains unflashed.
