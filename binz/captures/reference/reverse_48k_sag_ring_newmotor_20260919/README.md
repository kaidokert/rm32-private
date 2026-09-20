# Replacement-motor 50% fast-sag causal diagnostic

ELF SHA256 `796707248532CA4A700711A9092A7F1A649E9987241C418A105D05CD01E3B12C`.
Release-s/thinLTO/codegen1; text129672/data1200/bss17716. Same protected
reverse48k/cache-owner candidate as `reverse_48k_cache_owner_20260919`, plus
only opt-in `bench-fast-sag-causal` (eight raw scans, accepted-event ring,
terminal PWM snapshot; post-stop dump). Four motor ISR-root math audits PASS.
Explicit G071 flash/verify/reset and disabled guard3/18, roledu1006/6,
duty6/6, idle/final p/i off/nFAULT high PASS.

One protected 10->25->40->45->50% A/B ACKed all rungs, then fast-bus reason26
at powered19.956141s, with no foldback, tracking or nFAULT stop. The frozen
1.8ms trace is retained at
`captures/reverse_direction_2026-09-19_new_motor_48k_sag_ring50.txt`.
The bus trends lower and phase codes approach both ADC rails; accepted events
continue with several longer gaps. Causal onset remains unresolved. Diagnostic
image is currently installed, motor outputs off, COM41 closed. Not a lean
qualification or clean50 hold.
