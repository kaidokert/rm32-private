The supplied test log reports 10 passing tests; both ELF audit logs report all four roots clean. These are reported results, not independent verification of the binaries.

The corrections address the stated holes:

- Restricting encoding tokens to 2/4/8 hex digits preserves the three-letter `add` mnemonic.
- Reachable PC destinations, non-return multiple loads containing PC, and unknown decoded opcodes set `indirect`, causing failure unless the containing function is allowlisted.
- `neg`/`negs` are explicitly recognized.
- Entry traversal retains live blocks after returns while excluding unreachable padding.

No actual artifact blocker is demonstrated by the supplied material. One evidence limit remains: the logs do not show the invocation, allowlist, or disassembly, so they cannot establish whether certification depended on exemptions.

The fixtures support the specific parser and transfer fixes; they do not establish broader control-flow completeness. No powered testing is needed for this review.
