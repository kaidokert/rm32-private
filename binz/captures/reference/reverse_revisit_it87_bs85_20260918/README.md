# Reverse-only aligned origin/interval/bus diagnostic (2026-09-18)

Frozen `release-hybrid` ELF SHA256
`BBCC4F564C5564ACA46CAB01FC755B82189B6322808237740C9B9DA056245024`;
opt-s/thin-LTO/codegen1, text126268/data1208/bss18480. Base is the
reverse-only dispatch-origin control (4 A active current limiter, 5%/three
coherent 226-us scan fast-sag stop, nFAULT/tracking/watchdog/absolute bus
stops, ordinary startup/restart) plus `bench-revisit-origin-tail`. That feature
adds the existing 128-event interval suffix and 16-frame bus suffix, packing
the two origin bits into the upper bits of the already-recorded event limit.
The wire tag is IT87; previous IT86 captures retain their decoder semantics.
No live serial output or control/protection decision changes. The accepted
recorder symbol is 84 bytes larger than the preceding origin-only diagnostic;
this is an observer-affected image, not lean qualification.

Origin Rust tests 3/3, IT85/86/87 decoder tests 6/6, bus decoder tests 3/3,
old aligned IT86/BS85 capture replay, and four ISR-root forbidden soft-math
checks pass. The latest ELF was only frozen, not flashed, when written.

Subsequently flashed and run once to ACKed40%; retained capture
`../../reverse_direction_2026-09-18_it87_40_adcfault.txt`. It stopped on
existing ADC/DMA reason11 with a phase-rail frame, not fast-sag reason26.
The exact reverse lean image was then restored and verified OFF.
