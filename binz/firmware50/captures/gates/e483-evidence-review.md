The supplied source fixes the check/write race for the three reviewed callers, conditional on guard-stop exclusion: `powered_write` reads both admission flags and executes the write inside the same `interrupt::free` closure. A completed stop refuses the write; a guard deferred until after the transaction can clear it afterward. `apply_plan`, `moe_on`, and nonzero `set_compares` all use this wrapper.

It does not inherently block restart. All-zero compares bypass admission, and `Armed::start` calls `guard_arm()` before guarded `moe_on()`. However, `guard_arm` and `sine::compares` implementations are absent: this packet cannot independently establish that rearming clears the latch/sets guard active, or that startup’s compare calculation returns `[0; 3]`.

Coverage gaps:

- The foreground model covers stop-before, stop-after, inactive refusal, and a stale-admission negative control. Recomputing that control gives `moe=true, compares=133` after stop. It lacks a stop→rearm→successful-write case and an explicit successful-write assertion before the subsequent stop.
- Text checks establish token order and caller presence, not lexical containment, absence of additional bypass writes, or compiled exclusion semantics.

The log reports **415 passed, 0 failed** despite an incremental-cache warning. Audit names archived `869B7E4E.e483-stop-write.elf`; the diff reports unchanged ISR instructions, with relocated constants in two roots. Neither establishes foreground machine-code behavior or source-to-archive identity. No powered-run evidence is supplied.
