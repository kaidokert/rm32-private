# AM32 deep-study index (2026-07-17)

Three Opus source studies of E:/m/robot/esc/AM32 (uart_control branch,
stock mechanisms). Full texts lived in the research-agent outputs; the
actionable synthesis is GAP_CLOSING_PLAN.md. Key mechanism facts, with
source anchors, for re-verification:

## Study 1 — ZC chain (the two-timescale keystone)
- Gate: COMP IRQ accepts only when INTERVAL_TIMER->CNT (0.5us ticks,
  TIM2 PSC=39, reset ONLY at accepted ZC) > average_interval/2
  (stm32l4xx_it.c:276-290). average_interval = 6-commutation rolling
  mean (main.c:2098,2222) - a single accept moves it ~2%: UNWALKABLE.
- Schedule: commutation_interval = (old + (last+this)/2)/2 (0.25 gain,
  main.c:893); waitTime = interval/2 - advance armed at the ZC into
  TIM16 (0.5us one-shot); advance in 0.9375-deg units.
- Persistence: filter_level = map(average_interval,100,500,3,12),
  forced 12 while zero_crosses<100, floor 2 below 25us interval
  (main.c:2372-2379); loop bails on FIRST pre-ZC-level read.
- Premature accept (190us into 345us): passes the gate, moves the
  estimate only -11%, gate moves -2%, real ZC still lands in window;
  recovered in 2-3 commutations. NO phase-walk possible because the
  gate reference cannot be dragged by the accept.
- Stall immunity: persistence rejects most PWM ripple; estimator
  damped; gate stiff; 45000-count (22.5ms - TIM2 is 0.5us!) BEMF
  timeout forces polling re-lock; desync_check (>50% avg jump/rev)
  trips within one control tick; interval seeds (12500/10000/9000/
  5000) at every transition.
- Deaf window: mask from accept to commutation, then gate rejects to
  avg/2 - structurally listens only in the SECOND HALF of each window.

## Study 2 — Recovery lifecycle (kills vs re-seeds)
- Two modes: interrupt (comparator) above ~667Hz; polling
  (old_routine: getBemfState at 20kHz, min_bemf_counts=2 (+1/x2 for
  the first 5 crossings), bad_count tolerance 3) below. Polling is
  MATHEMATICALLY IMPOSSIBLE above ~667Hz (50us cadence vs sub-45us
  post-ZC half-windows) - the top end is protected by gate+damping
  ONLY.
- ZERO kill paths where minz has four. Missed ZC: ride (phase stays
  energized until the late crossing re-anchors). Desync (>50% avg
  jump, main.c:2223): zero_crosses=0, old_routine=1, duty CUT to
  min_startup_duty/2 = 3%, hot-restart - never a coast/latch. 22.5ms
  timeout: forced commutation + polling + zero_crosses=0. Only
  stuck_rotor (bemf_timeout_happened > 10/100 strikes, amnesty at
  zero_crosses>1000) actually stops the motor.
- Startup: level-triggered self-paced poll at pinned ~6% duty,
  wait-skip for first 5 crossings, handoff at interval<250us. No
  scheduled instant = no lottery.

## Study 3 — Duty governance (the di/dt clamp + variable carrier)
- NO input smoothing for DSHOT; ALL shaping is the 20kHz slew clamp:
  max_duty_cycle_change in {2,6,16}/2000 per 50us by regime
  (startup zero_crosses<150 or duty<7.5%: 2 -> 2%/ms; interval>500us:
  6 -> 6%/ms; else 16 -> 16%/ms), SYMMETRIC accel/decel
  (main.c:1688-1708). 50->70% at 1900Hz = 1.25ms linear ramp of
  0.8%/50us micro-steps. minz: 1% instantaneous CCR edges held 50ms,
  no firmware clamp - each edge is bigger than anything AM32 applies.
- variable_pwm is NOT vestigial: main.c:2131 tim1_arr =
  map(commutation_interval, 96, 200 [0.5us ticks = 48-100us],
  ARR/2, ARR): carrier 24->48kHz with speed; ~27kHz at 1900Hz. The
  empty block at main.c:1714 is a decoy. (CARRIER_REQUAL erratum.)
- Blind window: NOTHING cuts duty for 22.5ms (rides at last duty);
  the duty cut happens on desync DETECTION, not on miss. minz's
  blind-amp clamp (2/3 at ~270us) is ~83x more eager and is itself a
  torque-step disturbance.
- Low-RPM duty cap (map k_erpm 20..70 -> 20%..100%) fully open above
  ~1167Hz - NOT transit-relevant at the bench point.
