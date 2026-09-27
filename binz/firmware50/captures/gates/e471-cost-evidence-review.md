- **The supplied run fails before CFG validation or arithmetic.** `mnemonic.startswith("b")` misclassifies `bics` at `0x08009140` as a branch. Its register operands yield `target=None`, triggering `shutdown cycle/escape`. The same defect affects `0x08009166`. This is a checker defect, not evidence of a shutdown escape.

- **Raw callee coverage looks complete:** `0x08009110–0x08009176` contains 52 instructions, one forward `bne`, no calls, and a returning `pop {...,pc}`. It is acyclic, though not literally straight-line.

- **The displayed mask CFG appears closed:** refusal paths converge on `0x08000ae4`; the literal pool is bypassed; the persistence back-edge is `0x080008f0 → 0x080008d8`. The nested restore at `0x08000ad6` restores the already-masked state.

- **Twelve iterations require the depth invariant.** The loop compares against a loaded byte, without a local twelve-read clamp. Initialization and the displayed refresh support that invariant; `DepthSnapshot(pub u8)` alone does not enforce it. Thirteen listed loop instructions correctly include the refusal branch.

- **Arithmetic:** `1930/64 = 30.15625 µs`, but the supplied log establishes no cycle total. `charge()` also adds eight unnecessary cycles across the two `bics` instructions.

**Next offline action:** use explicit branch classification, rerun callee/CFG checks, and regenerate component totals. Keep snapshot exclusion conditional on the exact resume predicate. Hardware WCET and pending safety-service latency remain unbounded; no hardware authorization.
