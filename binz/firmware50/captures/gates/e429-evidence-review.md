The supplied source and assembly support fixed-16 fresh-estimate scheduling, but not every claimed equivalence check.

- `ConstantAdvance<FreshEstimate, 16>` binds the handler’s level; both compile-time assertions bind the foreground policy endpoints to 16. The failed build demonstrates rejection when `ADVANCE_LOW != 16`. This is sufficient for the shown `AdvancePolicy`, provided `Production` actually selects it.
- Assembly stores the newly blended, clamped estimate at `0x08000926`, then computes `(ci >> 1) - (ci >> 2)` at `0x08000932/946/948`, retaining saturation handling. This matches the reference integer fixed-16 formula; substituting `ci >> 2` for the entire wait would differ for some intervals.
- The emitted path retains arm eligibility checks and the expired-wait stop branch. No concrete safety regression is apparent in that path.
- Coverage remains incomplete: wrapper implementation, root-side advance selection, test contents, and comparison binaries are absent. The reported 379 passes establish suite success, not exhaustive equivalence. The four-root identity summary lacks identified comparison endpoints.
- The shifts/subtraction claim is supported. The claimed 813→787 reduction is not independently established here, and instruction counts do not establish WCET.

This supports proceeding with exact-image verification and disabled-output boot checks. It does not alone justify the powered single-15 screen.
