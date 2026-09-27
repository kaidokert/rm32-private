Keeping the generic entry is justified for this bounded change: E478 restores the archived motor loadable hash and clears the reported ISR audit finding. Concrete objections apply to broader claims:

- **Probe evidence does not transfer automatically.** E478’s probe hash differs from D677’s. Previous probe execution results do not validate the rebuilt probe; no E478 firmware was flashed. Its own ELF and runtime behavior need independent validation before claiming equivalent diagnostic coverage.
- **The compiler explanation exceeds the supplied evidence.** The controlled source delta supports attributing the observed build difference to entry genericity, but hashes and reachability summaries do not establish the specific “ahead of LTO” mechanism asserted in the comment.
- **Isolation is artifact-scoped.** Restored motor bytes support equivalence to C2 for the compared loadable content, inheriting C2’s validation limits. They neither establish source-level isolation across configurations nor guarantee future compiler/build outputs remain unaffected.

These limits do not require reverting the generic entry.
