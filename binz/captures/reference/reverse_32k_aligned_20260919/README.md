# Reverse-only 32 kHz aligned event/bus diagnostic (2026-09-19)

Frozen ELF SHA256
`AAE4E8FE6EE5EEC2C61D6EEE0617B282ED2BD3034CEDB9A9E1A1B4A71DEFBE05`.
The prior 32k fault-frame closure plus opt-in `bench-revisit-origin-tail`:
128 accepted-event IT87 rows and 16 coherent ADC BS85 rows, dumped only
after shutdown. Active4A current foldback/terminal, fast 5%/three-scan
relative bus stop, absolute bus, nFAULT, tracking, IRQ-rate and watchdog
remain. Diagnostic, not a lean qualification image. Release-hybrid opt-s,
thinLTO, codegen1; text126212/data1208/bss18480. Exact ELF four ISR-root
soft-arithmetic forbid audit PASS. IT87/BS85 decoder tests9/9 PASS.

Flashed/verified on explicit G071. Disabled guard3/18, reverse role6/6,
duty6/6, outputs off/nFAULT1 passed. A protected reverse ramp reached
44.5% command and fast bus-sag stopped at powered36.774451s before45%
was accepted. Final outputs off/nFAULT1. Capture and decoded chronology:
`../../reverse_direction_2026-09-19_32k_it87_bs85_445_fastbus.txt`.
