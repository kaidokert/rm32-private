No concrete new blocker appears in the supplied selected lean path; conditional feasibility for the bounded screen is supported.

New `0x08000942–0x08000956` stores acceptance payload before the sequence increment; arming follows under retained PRIMASK protection (`0x08000968–0x08000a2c`). The persistence continuation restores the timer pointer at `0x08000b9e` before returning to elapsed-time accounting. Saved-mask stack slots remain consistent across arm, bypass, and persistence branches.

The unmasked load/add/store requires the stated sole-writer/no-reset invariant, including preempting handlers. The compiler fence does not establish a coherent multi-field snapshot. COM’s recorder-only ordinal is absent with NoChain; payload overlap was already possible before the old masked increment.

Code layout and the filter-loop branch changed, so instruction removal alone proves no timing improvement. The audit/tests support screening, not hardware qualification or the unlinked diagnostic twin.

Proceed only with the specified single 25%, 28-second screen after 120 seconds off, fresh flash and passing boot checks; unchanged stops, no retry. Missing peak-current/temperature sensing and RPM rating remain explicit limits.
