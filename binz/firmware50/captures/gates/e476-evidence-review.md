Raw evidence: E476 achieved only 4 ms closed loop and zero target hold, versus control’s 22,778 ms closed loop and 20,278 ms hold. The requested 28 seconds is not achieved exposure.

E476’s sector counts sum to 7 accepted events, matching accepted/zc/com counts, with zero coalescing or forcing. Its 13 driven accepts belong to acquisition. Control’s sector sum 253,883 plus 30,772 coalesced accepts equals 284,655; zc/com counts are separately 284,657.

Reason 8 accompanies a tracking-stale trip: 4,718,645 − 4,717,616 = 1,029 µs, exceeding 1,000 µs by 29. However, the post snapshot gives acceptance age 27,298 − 27,277 = 21 µs, implying acceptance at 4,718,634—11 µs before the trip timestamp. This exposes inconsistent freshness snapshots; it does not establish the race’s cause.

Current means are 196 versus 199 mA, but exposure differs and zero drift is −307 versus −273 mA. Applied CCR/period is approximately 9.98% versus 14.93%, despite identical 15% targets. No hold-current comparison exists. Lower short-run handler maxima prove no speedup; zero-hold rate fields are invalid evidence.

Poststop and separate postflight confirm MOE/CCRs zero, gates low, enable low, nFAULT high.

Verdict: early guarded shutdown, not sustained-operation qualification or improvement. Root cause and supplied-source linkage to the captured binary remain unproven.
