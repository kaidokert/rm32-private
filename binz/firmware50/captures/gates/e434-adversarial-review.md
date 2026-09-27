The supplied disassembly shows both helpers byte-identical, including literal pools; their COM/DET/DRV/GUARD addresses also match the maps. `NoChain` inherits `ORDER=false`, sets `ON=false`, and supplies no-op callbacks. These close the specific helper/default gaps; I see no concrete new defect here.

Provenance remains reported: filenames and maps do not independently establish the stated commit/build/archive linkage. The targeted test passes but establishes only the watch’s stale-latching contract. The allowlist documents bounds; it does not itself demonstrate the reported root-audit result.

I support advancing to disabled boot checks, then the proposed single screen conditional on those checks passing and existing guards remaining effective. Resolve one scope ambiguity before energizing: the audit states a 5.2-second energized ceiling, so “28s” must not imply bypassing that ceiling.

Recorder removal can change timing despite identical helpers. This supports bounded candidate screening, not startup-failure attribution, WCET equivalence, or reliance on the unverified PSU setting as measured current protection.
