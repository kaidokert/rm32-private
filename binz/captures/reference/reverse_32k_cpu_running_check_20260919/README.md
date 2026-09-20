# Reverse32k running-only CPU probe with valid disabled cost check

ELF SHA256 `355C1630944C897A6BD361327BEE4AFCB711397EE9DD985BB31A2DE76629E562`.
Same reverse32k fixed20 lean control/protection closure plus CPU
aggregate and running-only gate. In motor ISRs the meter returns before
timestamp/critical-section work until ACKed live duty >=35%. Disabled
`cpucheck` uses a compile-time test-only force path that exercises the
active meter without enabling gates; no runtime test flag is read by
the motor ISR path. All electrical guards remain unchanged.

`release-hybrid` opt-s/thinLTO/codegen1; text129300/data1200/
bss17212, flash headroom572 bytes. Exact ELF four motor ISR-root
soft-arithmetic audit exits0. Not flashed or motor-tested when frozen.
First use: flash verify/reset explicit G071, disabled `cpucheck`
must show active-path cost (mode1/2 > gate-off2us), guard3/18,
reverse role6/6, duty6/6 and p/i off/nFAULT1. Then one bounded
ordinary-start ramp to35-40%; stop at first electrical or tracking
fault. CPU results are observer-affected and cannot qualify lean.
