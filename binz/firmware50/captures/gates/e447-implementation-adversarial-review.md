No concrete implementation defect established by the supplied material.

- Predicate branches match Rust: unsigned `(requested − 1) > 5`, detector mismatch, or nonzero COM phase reject at `0x8004250`, restoring saved PRIMASK.
- Success pends at `0x800420a` before restoring PRIMASK. The nested mask restores the already-disabled state. However, `0x80041b8` branches to omitted `0x80042cc`; restoration on that path cannot be verified from this excerpt.
- This blocks stale steps after one commutation, assuming COM updates are excluded by PRIMASK. Six-step wraparound remains an acknowledged generation ambiguity.
- Added masked loads/comparisons increase foreground work. Unchanged ISR instructions do not establish unchanged interrupt latency, cycle cost, or WCET; relocated constants also prevent byte-identity claims.
- The 387 passing host tests support functional regression coverage. Predicate tests simulate observations, not interrupts; source-string ordering does not prove lexical mask scope or restoration. The incremental-cache warning does not negate the reported pass.
- Neither E446 causation nor the proposed screen’s safety is established.
