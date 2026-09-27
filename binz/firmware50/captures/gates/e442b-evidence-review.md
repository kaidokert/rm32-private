Conditional go for the specified bounded screen; no concrete new blocker appears in the selected lean path.

The persistence calculation survives: new `0x080008c2–0x080008d4` retains the multiply/add/shift/clamp sequence and constants; comparator sampling remains bounded. Its changed branch layout prevents inferring cycle savings.

Payload stores at `0x08000942–0x08000950` precede sequence load/increment/store at `0x08000952–0x08000956`, which precedes arming. The old sequence-only PRIMASK wrapper is removed; arm protection remains at `0x08000962–0x08000968`, with restoration on bypass and completion. Separate stack scalars preserve the saved mask through those paths.

Correctness remains conditional on ADC_COMP being the sole sequence writer, without concurrent reset or re-entry. This is not coherent multi-field snapshot publication; the earlier payload exposure already existed. COM’s recorder-only ordinal supplies no lean-path blocker. The unlinked diagnostic twin receives no hardware qualification.

Proceed only with the exact identified image, 120-second off period, fresh flash, passing boot checks, and one 25%/28-second screen with unchanged stops and no retry. Missing peak/temperature sensing and RPM rating limit conclusions to that screen.
