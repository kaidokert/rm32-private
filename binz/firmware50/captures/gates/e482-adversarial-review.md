**Concrete defect: the handover stop check does not protect subsequent writes.** In `Handover::lock`, the guard can trip after `latched` is read. `det_install` and `com_handover` then refuse activation, but `set_period` and `apply_plan` still execute, potentially overwriting the guard’s zeroed compares. MOE/ENABLE remain down on this path; this proves a shutdown-state overwrite window, not re-energization or the cause of E481.

A stronger startup ordering concern exists in `Armed::start`: `guard_arm()` precedes unconditional `moe_on()`. A guard interrupt between them can shut down, then foreground reasserts MOE. ENABLE remains low, but the shutdown invariant is violated.

Remaining concurrency uncertainties:

- `Board::guard_arm` enables TIM6 updates before `roots::guard_arm` initializes state and clears NVIC pending. Peripheral-update clearing and actual delivery ordering need verification.
- Timed `STATE.schedule` survives stop. `after_phase_observed` replaces it on phase 1 and checks authority, but pending-IRQ cleanup depends on omitted timer helpers and `revisit_delivery`. A clean first dispatch is not established.
- `publish_accepted_arm` publishes fields/sequence before arming and subsequently calling `guard_event`. Guard preemption can therefore observe an overdue watch despite a newly published acceptance. Atomic fields do not make that transaction atomic.

E481 proves a stale-watch stop after six accepts, **not retained-state corruption**. Sequence continuity is intentional; `applied_ccr=133` indicates approximately 10% duty, not attained 25%.

Changing entry carrier is justified as a bounded experiment with unchanged thresholds and predetermined attempts. It tests carrier-transition sensitivity; improvement cannot distinguish analog effects, first-sector timing, and concurrency timing without corresponding traces.
