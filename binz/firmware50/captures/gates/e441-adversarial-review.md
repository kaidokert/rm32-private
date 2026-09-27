Conditionally sound; the supplied source supports the replacement, but does not establish the restored tree’s complete writer set.

- Both increments execute in mutually exclusive COMP decision paths. With nonrecursive ADC_COMP and no concurrent reset/store elsewhere, relaxed load/wrapping-add/store preserves the counter’s behavior, including wrap. Removing masked RMW permits higher-priority interrupts between load and store; the shown COM/guard paths do not modify `accept_seq`.
- Publication remains unresolved: `accept_raw`, `accept_avg`, `accept_blank`, then sequence are separate writes before the arm’s critical section. Under `com-top`, an already-pending COM can preempt that publication and consume a mixed snapshot or associate new acceptance metadata with an older arm. Neither the existing RMW nor its replacement prevents this.
- Foreground can be interrupted between sequence and raw reads. Correct pairing depends on the omitted `accepted::observe` implementation.
- A private helper restricts increment calls, not writes to the exposed atomic. Source-pattern tests cannot establish exclusive ownership across resets, aliases, features, or restored files.

Review the actual restored diff and generated instructions before approval; unchanged sample spacing is unproven.
