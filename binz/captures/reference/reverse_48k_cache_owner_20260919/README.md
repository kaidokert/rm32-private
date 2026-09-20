# Reverse 48 kHz phase-rail cache ownership A/B

SHA256 `42C9D49E3C8494D926341D08D39416808D52B2F2D0360F59FCCF4BE3B073154F`.
Release-s/thinLTO/codegen1, text125588/data1200/bss17200.
Same feature closure as the preceding fault-frame diagnostic SHA69BB6A10,
with one source policy change in `startup_feedback::Latest`: physical phase
codes 0/4095 are cache-valid because the DMA signed-current owner already
includes and counts them. Impossible >12-bit phase words, bus/VREF rails,
stale/reordered frames still latch. The 4 A signed-current foldback/hard stop,
fast 5%/three-scan bus stop, nFAULT, tracking and watchdog paths are unchanged.
This is an exploratory A/B, not a lean qualification image.

Pure cache policy tests 5/5 PASS; four motor ISR-root soft-arithmetic audits
PASS. Explicit G071 flash/verify/reset, disabled guard3/18, roledu1006/6,
duty6/6 and idle/final p/i off/nFAULT high PASS. A protected new-motor A/B
ACKed25/40/45/50%; one exact phase rail was counted by the current owner
without a cache stop, proving the changed path executed. It then stopped on
the independent fast-bus three-scan reason26 at powered20.019369s, after50
ACK; no current foldback, tracking or nFAULT. Final average185 half-us
(~1801eHz) vs ACK interval172 half-us (~1938eHz) indicates a lower speed at
terminal than the50 ACK, but the ACK is not a full stable-speed record.
No clean50 hold. COM41 closed. The prior new-motor image
reached/ACKed 50% but stopped on a single phase exact rail at 50%; its
terminal sample also included one ~8% low bus code. This change does NOT
prove that sample was noise or that phase peak current is safe; it removes a
cache veto that contradicted the explicitly selected current-owner policy.
The fast-bus stop is the active50% blocker on this one A/B.
