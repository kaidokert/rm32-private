**Achieved duty and dwell differ from requested duty.**

| Capture | Achieved PWM duty | Target dwell | Stop |
|---|---:|---:|---|
| E393 | 500/1000 = **50.000%** | **19.778 s** | Deadline, reason 2; closed-loop duration 39.778 s |
| E394 | 590/1000 = **59.000%** | **0 s at 60%** | Sag, reason 26; closed-loop duration 24.559 s |
| E383 | 746/1333 ≈ **55.964%**, nominal 56% | **0 s at 60%** | Sag, reason 26; closed-loop duration 23.153 s |

E383’s denominator is inferred from the supplied 48 kHz context. Neither fault completed its requested 45 s. Exact elapsed powered stop timestamps are absent; closed-loop durations must not substitute for them. Zero-hold rate/current fields are placeholders, not measurements.

**Ordinal and terminal ordering:** E394’s acceptance offset is `727554−252343=475211`; its COM offset is `727554−252344=475210`. These match E393’s preceding totals. Normalize each stream separately; local acceptance and COM indices differ by one even when matching boot ordinals identify the corresponding transaction.

Using local microsecond stamps:

- E394: a **107 µs accepted gap ends at 14843 µs**; bus remains normal at 14885 µs. Phase-A code **55** appears at 14990 µs. The terminal raw-bus sub95% sequence begins at 15503 µs; mean-based streak 1/2/3 occurs at 15802/15891/16027 µs.
- E383: accepted intervals extend to **76/83/83 µs**, ending at 41164/41247/41330 µs; phase-C code **76** appears at 41442 µs, phase-B **0** at 41547 µs. Terminal raw-bus sub95% starts at 41847 µs; mean streak starts at 42081 µs and trips at 42243 µs.

Trip means normalize to approximately **94.29/93.63/92.06%** in E394 and **93.64/92.98/91.82%** in E383, against each row’s reference. Thus this packet’s guard judges **eight-scan means**, not three individual raw samples.

Fine timestamps are **0.125 µs/tick**, wrapping every **8192 µs**. Last recorded bridge-write completion precedes trip recording by **8.375 µs** / **182.25 µs**, respectively; these are software stamps, not physical gate/crossing times. Coast estimates near **2744/2736 electrical Hz** undermine interpreting terminal accepted-interval estimates as rotor deceleration.

ADC rails indicate clipping; asynchronous samples cannot establish peak/RMS current. Offset drift and exclusion of the trip block further limit current proxies. PWM-counter reads cannot locate acquisition apertures.

The lean64 proposal is reasonable exploratory screening, but **n=1**, changing observers and binary layout, cannot establish mechanism. E393/E394 compare targets on one image; E383/E394 are unreplicated carrier comparisons. Cheaper first: exploit this existing ordering analysis. For carrier attribution, matched **lean48 versus lean64** is cleaner. Retain fault termination, **3 A**, cooldown and guards; absent temperature sensing, neither thermal nor loaded qualification follows.
