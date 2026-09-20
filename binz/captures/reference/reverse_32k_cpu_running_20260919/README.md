# Reverse32k running-only aggregate CPU diagnostic (retired before motor use)

ELF SHA256 `922166D180B8795A3679B3C23C5E3E8CAE8666D92158E11B092C37BB02BB4CA6`.
Same reverse32k fixed20 lean control/protection feature closure as
frozen SHA06B1C1D6..., plus `bench-cpu-running-only`, whose dependencies
are exactly the aggregate CPU meter closure. The meter's ISR scope
returns before timestamp/critical-section work until the live request
is ACKed at >=35%; startup/seed/handoff remain electrically unchanged
apart from a small read-only gate. Once armed, CPU union/root accounting
is observer-affected and cannot qualify the lean envelope. The compiled
feature fingerprint includes `bench-dma-peer`, matching the frozen lean
build; DMA is a COMP/COM peer at priority64, TIM6 stays0.

`release-hybrid` opt-s/thinLTO/codegen1; text129112/data1200/bss17212,
flash headroom760 bytes. Exact ELF four motor ISR-root soft-arithmetic
audit exits0. Electrical stops are unchanged: active4A signed-current
foldback/stop, fast5%/three-scan and absolute bus, nFAULT, tracking,
IRQ-rate, watchdog. It was flashed/verified/reset and passed disabled
guard/role/duty/off checks, but its disabled `cpucheck` exercised the
gate-off path only (2/2/2us). This cannot calibrate active meter cost.
No motor run occurred. Superseded by
`../reverse_32k_cpu_running_check_20260919/`, whose disabled check
exercises the active meter while gates remain off.
