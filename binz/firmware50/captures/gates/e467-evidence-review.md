Admissible as an offline refactor; equivalence and instruction savings remain to be demonstrated.

- Raw loop `0800084c..864` executes **12 instructions per matching attempt**, including the literal-address load, one register read, masking, four Boolean-normalization instructions (`subs/sbcs`, `rsbs/adcs`), comparison, and loop control. A mismatch executes **10 instructions**, including the exit branch at `85e`. Instruction counts are not cycle counts.
- Precompute `expected = if rising { VALUE_MASK } else { 0 }`; each attempt must evaluate `(volatile_register_read & VALUE_MASK) == expected`. Comparing a masked register directly with Boolean `0/1` is incorrect. Preserve the existing non-inverted polarity convention.
- The matching API must invoke its callback only after passing the strict gate (`count > blanking`), exactly once per attempt, stopping immediately on dissent. Depth zero performs no read. Constructing the closure must not sample hardware.
- Required differential coverage: both polarities; gate equality and adjacent counts; depths 0–255; dissent at every attempted position; all-match sequences; repeated accepts/refusals; bounds and extreme intervals; Fresh/Previous estimates and runtime/constant advances. Compare outcomes, callback counts, estimator history, and counters. Verify hardware-mask semantics independently.
- Caller coverage must preserve rebase behavior, snapshot validity, publication-before-arm, and stop handling; include logged and plain paths. Inspect resulting disassembly for actual savings and retained volatile reads.

Faster sampling changes physical persistence duration. Logical equivalence cannot transfer bench qualification.
