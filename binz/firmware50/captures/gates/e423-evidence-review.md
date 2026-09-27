The concrete preflight gap is runtime verification: the supplied evidence contains no boot/self-test result from the staged image. The `p` response is a literal PASS string; it does not rerun tests or independently verify ENABLE-low. Reaching it establishes that the startup predicates passed, assuming this exact image is running. Pad/ENABLE verification needs separate evidence.

Recorder findings:

- ACC and COM have separate root-owned rings; the shown wiring introduces no evident shared mutable row.
- `OrderSag::freeze()` exists, but its integration is not shown. This main explicitly disarms after `controller.command()` returns. Therefore, these snippets establish command-return freezing, not freezing at the stop decision.
- Disassembly supports diode-mask application followed by two genuine TIM2 reads bracketing TIM17. That bracket timestamps a **post-application read**, not the bridge transition.
- Accepted-only tails cannot establish rejected crossings, physical crossing accuracy, or lean timing. Independent tail truncation also requires joining by ordinal, not row position.

The reported tests and ISR audit support their respective software checks, not bench preflight or WCET. No demonstrated recorder defect here independently invalidates the proposed conditional bounded screen.
