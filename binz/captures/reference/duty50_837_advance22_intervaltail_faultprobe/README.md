# E837 fixed-advance-22 timing experiment

Third one-variable fixed timing point after advance16/18/20. Advance22 remains
inside stock AM32's auto-advance range (13..23). All other E835 diagnostic and
protection selections are unchanged.

- ELF SHA256: `A742F7828E3046213546F37267D2F46130A714955ACD792722D490378DF7D829`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122132, data1180, bss23680
- minz-core advance22 cohort: 86/86 PASS
- four motor ISR-root M0 arithmetic reachability audits PASS
- diagnostic-only; not envelope qualification
