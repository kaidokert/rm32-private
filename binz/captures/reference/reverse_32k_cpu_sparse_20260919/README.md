# Reverse32k sparse per-vector CPU diagnostic (staged)

ELF SHA256 `FCC62B4BA7FBC7B8A83D93672AD603F214B4B92AEFD696F304876F5A6F27B523`.
Exact reverse32k fixed20 lean control/protection feature closure plus
only `bench-cpu-sparse`; fingerprint compared against the proven reverse
control build. DMA remains a COMP/COM peer at priority64, TIM6 stays0.
Current4A foldback/stop, fast5%/three-scan and absolute bus, nFAULT,
tracking, IRQ-rate and watchdog stops unchanged.

The diagnostic returns before clock reads below ACKed35% live duty.
At/above35% it counts each TIM6/COMP/COM/DMA entry and brackets only
every17th per vector. The first sample records one extended board
timestamp; `POWERPATH stop_us` supplies the end. Inclusive sampled
duration can contain higher-priority preemption, so sums are NOT IRQ
union and cannot be transferred to lean occupancy without caveats.
Use the per-root mean/max and a conservative upper bound, not a false
exact CPU percentage. No UART print during a powered window.

`release-hybrid` opt-s/thinLTO/codegen1; text126348/data1208/bss17200.
Four motor ISR-root soft-arithmetic audit exits0. No flash or motor run
yet. First use: explicit G071 flash verify/reset, disabled guard3/18,
reverse role/duty/off preflights, one bounded ordinary-start ramp to35%.
If a fault occurs, retain full terminal and do not retry for a pass.
