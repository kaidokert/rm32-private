sorry it seems like we have some hard caps here. If we expect to achieve 300hz, what do we need to do ? cut WCET of our ISRs by 1/3rd or more ?

● Assuming you mean 3 kHz electrical (55.6 µs window) — and it's more than 1/3, because two separate ISR costs both have to come down, and one needs restructuring, not just
  trimming. From the measured breakdown:

  At 3 kHz the window is 55.6 µs. The fixed costs today:
  - commutation ISR 22 µs = 40% of the window (and 40% CPU at that rate)
  - elapsed (ZC→schedule) ~15 µs
  - floor 8 µs

  The two binding walls and what each needs:

  1. Schedule budget = T/6 = 9.3 µs at 3 kHz, so elapsed must drop from 15 → ~7 µs (~½, not ⅓). That's the COMP-ISR path: persistence loop + estimator-before-the-read.
  2. The commutation ISR (22 µs) is the bigger problem — it's ~40% of a 55.6 µs window and it blocks ZC detection (shares priority 1 with COMP). But most of it is deferrable
  bookkeeping, per the earlier lever-#2 breakdown:
    - close_float_window (window record + bb + MAGPIE serialize): ~7 µs → move to main
    - entry + class bb: ~4 µs → move to main
    - schedule_us (LPTIM bounce + ARROK + delay(200)): ~6.6 µs → TIM16 one-pulse kills it
    - set_six_step + mux (the actual actuation): ~5 µs — irreducible

  So the commutation ISR can go 22 → ~8 µs (~64% cut) by deferring diagnostics + TIM16 — that also drops CPU from ~40% to ~15% and un-blocks detection.
  3. Floor 8 → ~2 µs — same TIM16 move.

  So the honest sizing: a uniform "cut ISR WCET by ⅓" gets you to roughly 2.4 kHz, not 3. Reaching 3 kHz needs:
  - elapsed ~½ cut (COMP path),
  - the commutation ISR ~⅔ cut — mostly by moving work off the ISR into main (structural, not just faster code),
  - and TIM16 replacing LPTIM2 (kills ~5 µs of schedule latency + drops the floor to ~2 µs).

  TIM16 is doing double duty there — it's the single highest-leverage change (cuts the floor and the commutation ISR's schedule cost). That's why it keeps coming up as the
  real move. Want me to add a "path to 3 kHz" section to the analysis doc with these targets, or start scoping the LPTIM2→TIM16 swap and the close_float_window→main deferral?
