# E814 rolling-50 adaptive 3.5 A diagnostic candidate

Staged only at freeze time. Exact E804 diagnostic controls/protections plus an
O(1), division-free rolling copy of the existing50-sample current quantity.

- ELF SHA256: `487E4E936570E9A1CB75E45715C9AA6C2D0108E9D0C93BE9B3E3B3BDD59A57B6`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121724,data1196,bss23260
- unchanged50-sample nominal current and bus guards, thresholds, nFAULT,
  tracking, watchdog and non-overlapping second-over terminal stop
- rolling warning evaluated every226us after the first complete window
- applied foldback clears the rolling history, requiring50 fresh samples at
  the new duty before another rolling warning
-32 module-policy tests per feature cohort PASS; four motor ISR-root M0
  arithmetic reachability audit PASS

This supersedes the retired E812 fast20 design, which E813 showed was
phase-sensitive and over-conservative (five foldbacks to22%).
