Raw evidence: phase4 performs no commutation; recheck state is COM-owned. Crossing preparation clears TIM16’s peripheral and NVIC pending state before installing phase1. `line_live()` measures IMR only; `CEN=0` alone does not establish whether an IRQ remains pending.

The proposal preserves ownership and appears live, provided these conditions hold:

- Skipping leaves schedule and phase4 intact. COMP refusal must invoke the typed helper after successful `comp_resume_powered()`, including refusals caused by rebasing.
- Under one PRIMASK region, the helper must revalidate guard reason, detector/com authority, stopped latch, phase4, IMR and CEN before pending TIM16.
- Acceptance, storm, overrun and shutdown must bypass rescue. Their retained schedule is inert until COM replaces or retires it.

Concrete races: COM expires during masked COMP persistence → skip → refusal → rescue. Acceptance instead replaces the expired recheck with phase1. Guard between comparator resume and helper must suppress rescue. COM between resume and helper may already rearm or exhaust the schedule; the helper must observe that updated state.

Shortest discriminating test: force phase4 expiry immediately after COMP’s raw stamp; verify no scheduler execution before refusal, then exactly one resumed observation. Repeat with acceptance and guard shutdown: neither may produce stale rescue or premature phase1 dispatch. Include an already-pending TIM16 case.
