Both helper disassemblies match instruction-for-instruction, including literal pools; the byte comparisons agree. Their shared-state addresses remain unchanged in the maps. This closes the presented stop/rearm helper-equivalence gap.

`NoChain` inherits `ORDER=false`, sets `ON=false`, and supplies empty callbacks. The bin diff removes recorder selection/control/output; the maps corroborate removal of recorder storage. This supports recording removal, not identical interrupt timing.

The targeted stale-boundary test passed despite the incremental-cache warning. It establishes only the stated watch contract. Commit/build/hash linkage and broader checks remain author-reported here; filenames alone do not independently establish provenance.

No concrete new defect is demonstrated. I support proceeding to disabled boot checks, then the proposed single screen conditional on those checks passing. Resolve one scope ambiguity: the audit describes a 5.2-second energized ceiling, so “28s” must mean a guarded campaign, not a continuous run bypassing that ceiling. The PSU setting does not establish measured current protection or thermal coverage.
