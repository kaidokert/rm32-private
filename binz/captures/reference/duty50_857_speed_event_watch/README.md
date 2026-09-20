# E857 speed-scaled missing-event watchdog diagnostic

Installed ELF SHA256
`9D8CD681636F59BCFBFBF4BA2F531972447891C5EB9D2477480E3E087EAF1520`.
Release `opt-level=s`, thin LTO, codegen-units=1; text122060, data1180,
bss23976. This is the E855 fixed-advance20/current-report-only/persistence
diagnostic plus the existing 128-event interval tail and one new opt-in guard:
after each accepted event, the fixed1000us missing-event deadline may tighten
to three controller event periods (bounded200..1000us) and never loosens during
a slowdown. Electrical, bus, nFAULT, feedback, deadline and tick guards remain.

Host policy sweep and wrap/deadline tests pass. The first implementation used
`saturating_mul(3)` and the full ELF scrub found `__aeabi_lmul` in the emitted
Recorder despite the four vector-root reachability audit passing; it was
retired before flash. The installed implementation uses a bounded const
function whose only product is <=1998. Rebuilt disassembly has no arithmetic
helper in Recorder/watch code; ADC_COMP, TIM16, TIM6 and DMA root audits pass.
Uninitialized controller intervals below64 half-us cannot tighten the deadline.

Disabled hardware checks pass: guard3/18 and role6/6 at2666 ticks; final p/i
show all six gates, ENABLE, MOE and CCRs low/zero, nFAULT high. No powered motor
run was made because E855's hard-fold pause remains in force. The image is a
diagnostic candidate, not qualification.

Reset note: the first CubeProgrammer invocation omitted the ST-LINK serial and
hard-reset a separately attached F411 (device0x431) only; it did not flash it.
The corrected serial-qualified invocation reset the intended G071 (device0x460).
