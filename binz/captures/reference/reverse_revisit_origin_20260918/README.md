# Reverse-only acceptance-origin diagnostic (2026-09-18)

Frozen ELF SHA256 `0178D6665A3AB0C917A56759F49F3191A72F588320201556C82C3DF725649782`.
`release-hybrid` (opt-s, thin LTO, codegen1), text 125560, data 1200,
bss 17176. Built from the reverse 4 A/fast-sag/normal-restart feature
closure with `bench-revisit-origin`; omitted the post-run latest-fault-frame
and reverse-seed-stage diagnostics. Four motor ISR-root soft-math audit PASS.
Host origin tests 2/2 and terminal/lean-report tests 8/8 PASS.

This image counts physical-only, software-only, both, and neither EXTI flags
at *accepted* comparator events, plus events more than 25% later than the
prior smoothed interval. Counters reset on a live duty change and print only
after power is off. Software-only is a lower bound on direct level rescue;
`both` is ambiguous because the software flag can survive a rejected dispatch.
The accepted-event path grows by 104 bytes versus the exact reverse lean
image. This is a diagnostic image, **not** a lean envelope qualification.

No old-direction motor runs are permitted. The prior exact reverse lean image
remained installed and OFF when this diagnostic ELF was frozen.

**Retired after test.** A 35% hold showed a false `neither` majority because
minz-core clears hardware EXTI before EV_ACC. The result is invalid as event
origin attribution. See the corrected dispatch-snapshot image in
`../reverse_revisit_dispatch_20260918/`.
