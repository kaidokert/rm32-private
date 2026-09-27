Raw evidence supports an incomplete ramp: `hold_ms=0`, with final applied CCR/period = 293/1333 = **21.98%**, below the 25% target. This establishes the reported final command, not measured effective motor voltage.

Ordinal matching gives **127 pairs**, one wide bracket (25 ticks = 3.125µs), and maximum recorded entry-to-postapply excess over wait of 8µs. Last paired acceptance 371696 occurs at 24040µs; postapply is 24059µs: 19µs elapsed against wait13. Terminal acceptance 371697 occurs at 24084µs, interval44/average44/wait11, without a matching commutation. `ci_at_late=44, spent_at_late=11` corroborates exhausted slack, but the aggregate late-arm record lacks an ordinal proving that association. Reason28’s label requires its definition.

The author’s bounded conclusion is reasonable: fresh remains unqualified. These observations establish neither valid physical crossings nor timing-policy causality; frozen-input tests cannot predict feedback behavior.

**Repeating both is not a prerequisite** for a bounded cost-only optimization. Rate comparison would require matched exposure and repeated trials; these unequal single runs cannot rank policies. First verify the optimization preserves decisions and reduces worst-case pre-arm cost. Hardware qualification remains outstanding.
