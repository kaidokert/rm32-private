# E846 scheduled-advance exploration image

Exact E804 adaptive3.5A exploration composition with advance18 replaced by
the scheduled18/20/22 policy. Unlike E842, this omits the accepted-event ring
and nFAULT classifier. Unlike E844, the known single-cycle floor is report-only
and event100/average post-run diagnostics are retained. Real signed-current
and bus protection, adaptive foldback, raw feedback, nFAULT, tracking,
deadlines, and watchdogs remain active.

- ELF SHA256: `9174CBDD2C28E897D1F34CEB64F3F0D3F24C9CF27CE3AF86D3D79AC2256F6E99`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121604, data1180, bss23156
- scheduled core cohort86/86 PASS; four motor ISR-root M0 audits PASS
- exploration only; a separate lean image is required for qualification
