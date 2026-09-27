Raw disassembly supports 12→7 executed instructions per successful persistence iteration: four Boolean-normalization instructions disappear and the literal-address load moves outside the loop. For depth N>0, those regions execute 12N versus 7N+1 instructions, excluding other setup. Full-root 855→849 and stack 68→76 cannot be independently established from these excerpts.

The model decreases by 235 cycles, 6.48%, or 3.672 µs at 64 MHz. This is neither measured latency nor WCET: fetch costs are omitted, bounds are supplied, and `loop-hit False` needs explanation.

Tests enumerate 197,376 depth/polarity/dissent cases and 3,072 history offers across three estimator types, plus 12 register comparisons. However, both APIs now execute the same implementation: agreement does not independently establish equivalence to the previous implementation. Explicit read-count and acceptance assertions remain useful. The register test duplicates the expression rather than exercising `ExpectedLevel`.

Masked equality is algebraically sound given VALUE bit 30 and an expected value of zero or that mask. No concrete logic defect is apparent. Physical equivalence is unproven: faster sampling shortens persistence duration and can change acceptance on identical waveforms.

The transcript reports 411 passes despite an incremental-cache warning; release/clippy evidence is absent. “Identical” ISR output needs identified comparison endpoints.

Retain as an offline candidate. Next: establish artifact provenance, compare against an independent pre-change oracle, validate loop bounds, then measure cadence and rejection behavior in a bounded lower-rung screen.
