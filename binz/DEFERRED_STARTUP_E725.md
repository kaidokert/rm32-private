# E725 — remove pending-IRQ camping from startup

Source evidence: minz/core/src/am32_isr.rs70-84 intentionally retains pending
EXTI when the timing gate is closed and the comparator is at the post-crossing
level. Repeated IRQ entry waits for the half-interval gate. The startup adapter
previously relied on64dispatches/ms to stop this. Removing that quota exposes
the busy-dispatch mechanism. This does not prove the cause of E724's silent
failure, whose live state was lost during reset recovery.

Timed startup now masks and clears an early post-level input and retains a
deferred obligation for the current command. The existing50us startup timer
can resume it only under the live driven owner, nonzero average and strict
`count > average/2`. It checks post-level again before sending a software EXTI
wake; if the level reverted, it simply reenables hardware edge sensing. A
command/mux change or reset cancels the obligation. A stopped owner cannot
resume. No comparator qualification, acceptance, interval reset, commutation
or output authority is performed by this timer helper. The ordinary COMP ISR
still executes reference persistence and measured-seed validation.

Default reference adapter behavior is unchanged. Average current, bus, fault,
tracking and watchdog checks are retained. Timer-mediated revisit introduces
up to one startup timer period of nominal dispatch quantization, plus execution
latency; no measured latency bound or production parity claim is made.

56 host policy tests pass. The new test covers the pure admission predicate
(owner, deferred state, zero average, strict half-interval boundary), not actual
NVIC/EXTI semantics or all interleavings. Release-s/thinLTO build and TIM16
arithmetic-helper gate pass. Frozen candidate:
`captures/reference/deferred_startup_725/shell-pwm.elf`, SHA256
`befec320e132d181d5b54c3031327bf1c3ea34de672ad113928abd3b4d6282bf`.

NOT FLASHED. No UART/SWD/motor actions this entry. Actual board remains restored
723d, last-off verified in E724. Next is disabled hardware source/mask/retry/
cancellation testing before a powered candidate run. Silent-failure cause is
still unproven; this is a concrete containment improvement, not a successful
spin or completion of the30% goal.
