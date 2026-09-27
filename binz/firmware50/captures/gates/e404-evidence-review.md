**The algebra is sound conditionally; the causal hypothesis remains unproven.** Let \(t_0\) be COMP entry, \(t_s\) the final preparation stamp, and \(W\) the requested wait, all in microseconds. For unexpired arms, program \(D=\max(2,W-(t_s-t_0))\), with ARR \(=D-1\). Actual commutation still includes poststamp setup and COM service latency. Verify raw-clock units, subtraction width/wrap behavior, timer clock, and supported delay range; these excerpts do not establish every premise. At 64 MHz, 1000 ticks means 64 kHz and 1333 ticks approximately 48.012 kHz.

The existing code subtracts elapsed time **before** entering the arm helper; subsequent preparation therefore adds uncompensated delay. Moving subtraction later addresses that mechanism. However, suppressing expired arms changes behavior: currently they receive a minimum two-microsecond timer delay. “Counted for existing stop” is unsupported here: the shown late-arm branch records statistics, with no stop action. Also clarify whether `sched_raw` records the requested deadline or predicted minimum-clamped expiry.

**Raw evidence:** one 60% run completed its deadline with 14.778 seconds reported target hold; one 70% run tripped sag after 9.025 seconds reported hold. Neither is repeated qualification. Both report zero late arms; all-zero margin counters supply no timing distribution. `spent_max_us=12` excludes later preparation. Coast measurements broadly corroborate speed, not commutation phase accuracy. Saturated sector/phase counters cannot support precise distributions. Current remains a proxy. Different ELF hashes require a source/configuration comparison before calling this duty-only evidence.

The supplied TI constraint supports choosing 48 kHz within its recommended range; it establishes neither damage at 64 kHz nor the sag cause.

**Minimum tests:**

- Host checks for wrap, elapsed below/equal/above wait, minimum delay, ARR limits, both callers, and expired-arm shutdown.
- Verify pending-IRQ clearing, atomic stop refusal, publication ordering, and unchanged relative arms.
- Measure requested deadline versus timer expiry/COM entry with bounded instrumentation.
- Compare baseline and candidate at **fixed 48 kHz**, identical guards and conditions, using a predefined cohort. Changing carrier simultaneously confounds attribution; 50% then conditional 60% is screening, not proof.
