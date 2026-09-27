Bitfield oracle is internally coherent: 22 = active + timer running + off; 18 = active + off; 17 = stopped + off. All expect TIM16 pending cleared, including case 4—this depends on the omitted acceptance/arm implementation.

Ordering is correct for `off_before` and `gates_before`: both precede cleanup. `BEFORE` is inside PRIMASK; `AFTER` follows window return, allowing pending higher-priority guard service.

Priority assertions use actual NVIC readback and require guard 0, COM 64, COMP 128; no measured values were supplied.

Concrete blocker: exact flashed-artifact verification and passing runtime records for all five cases, including pre/post readbacks. Build/clippy claims and the supplied audit do not establish those outcomes or Vref stability.

Scoped limitation: pending-during-window coverage establishes neither all stop interleavings nor production WCET.
