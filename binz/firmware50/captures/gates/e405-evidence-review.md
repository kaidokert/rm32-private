```rust
hw::com_timer::prepare_crossing();
let spent = hw::clock::raw().wrapping_sub(raw) as u32;
let left = crate::oneshot::crossing_left(wait, spent);
if left == 0 {
    stop_expired_arm();
}
```

**Verdict: the core change is supported by the supplied source and COMP disassembly; the evidence does not yet clear the pre-flash gates.**

- **Implementation:** COMP masks interrupts at `0x080009e4`, checks stopped/active, prepares TIM16, then samples the clock at `0x08000a24`. The positive path writes schedule/purpose, ARR, UIE and CEN before restoring PRIMASK. Exhausted/invalid waits branch to `stop_expired_arm` while still masked. Both supplied acceptance sites use the shared helper. This supports atomic post-preparation subtraction. The terminal callee’s machine code is absent, so its compiled shutdown sequence remains unverified here.

- **Arithmetic:** Saturating subtraction and rejecting waits ≥32768 are sensible. Positive remainder 1 deliberately becomes 2µs; poststamp instructions and service latency remain additional delay. However, restricting **wait** does not bound **elapsed**: a delay of 65536µs aliases to zero. Existing tests exercise wrap crossings with short elapsed times, not full-wrap ambiguity. Establish a maximum entry-to-final-stamp latency below one clock revolution, or use an independently sufficient elapsed-time mechanism.

- **Boot selftest integration:** `crossing_selftest_off()` masks TIM16 and never restores its NVIC enable state. The supplied entry point does not restore it either. Startup may unmask elsewhere, but that code is absent. **Before flash, demonstrate the normal handover explicitly enables TIM16, or preserve/restore the prior mask state.** Otherwise timer UIF polling can succeed while subsequent COM dispatch remains disabled.

- **Coverage:** The host tests provide useful arithmetic and ordering checks, but source-string searches do not prove critical-section scope or executable call coverage. No test-run output substantiates “365 host / 12 fixture / clippy passed.” No hardware selftest result exists yet; the printable PASS string is code, not execution evidence.

- **Gate exception:** The supplied ratchet explicitly fails: 742→778 instructions, +36 beyond ±8. The extra checks explain plausible growth, but do not establish acceptable masked latency or root cost. The audit reports success, but its implementation and complete reachable disassembly are absent.

**Minimum closure:** resolve TIM16 enable ownership; provide an elapsed-time bound, actual test outputs, and artifact-linked terminal-stop disassembly; document and explicitly accept the +36 ratchet exception with its timing limitation. Run the outputs-off selftest before powered screening.

E375 is a different-image 60% sag failure after only 320ms at target. It provides neither E405 qualification nor causal validation.
