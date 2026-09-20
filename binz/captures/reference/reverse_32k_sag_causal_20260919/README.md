# Reverse32k fault-triggered sag/controller diagnostic — 2026-09-19

ELF SHA256 `70CF7D0485025DF1DFB717C1E33749A596EB9A720201FB621C0918D90C080B13`.
Exact reverse32k fixed20 lean feature closure plus
`bench-fast-sag-causal`; release-hybrid opt `s`, thin LTO,
one codegen unit, text126116/data1200/bss17328. Four motor ISR-root
soft-arithmetic audits pass. All electrical stops remain active.

The diagnostic records only the latest one-to-three consecutive sub95%
bus scans, then prints after stop. It cannot prove event state at the
ADC bus aperture because controller fields are read at later DMA service.
Protected10%/15s and40%/60s gates passed; a stepped run stopped on fast
bus sag at44% before44.5% ACK. See `DUTY_50_CAMPAIGN.md` and retained
`captures/reverse_direction_2026-09-19_32k_causal*.txt` transcripts.
The exact fixed20 lean image was restored afterward; this diagnostic
is not installed and does not qualify the lean envelope.
