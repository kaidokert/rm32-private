The supplied captures report ten passing tests and clean audits for both CA94ACF6 and C2F903A0, across all four requested roots.

The corrections address the stated defects:

- Recognized instructions writing PC, non-return register-list loads containing PC, and unknown mnemonics now flag unresolved control flow.
- Pruning stops at those transfers; the retained instruction still triggers failure unless explicitly allowlisted.
- Restricting encoding tokens to 2/4/8 hex digits prevents consuming `add` as machine bytes.
- Both GNU `neg` and `negs` are accepted.

The tests also cover live blocks after returns, indirect tails, unresolved interior calls, and live versus dead loops.

**No artifact-specific blocker is demonstrated by this evidence.** The captures establish reported audit outcomes; without the ELF disassemblies, they do not independently establish complete decoding of those artifacts. Within the stated scope, the requested mov-PC closure is implemented. No powered testing is needed for this review.
