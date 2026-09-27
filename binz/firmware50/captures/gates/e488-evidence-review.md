Narrow admission is supportable; this capture provides no concrete reason to decline one 25% screen under the stated constraints.

The 15% run reached deadline, held target 20.277s, reported zero forced commutations and late arms, and passed both safe-off checks. One whole-run `thin_count` establishes a ≤2µs remaining-margin event; it neither locates that event nor establishes a hard-stop violation.

Numerators matter: sector accepts total 243,684; adding 48,364 coalesced accepts exactly recovers 292,048. Hold accepts use a different window. The powered tail spans 2.277110s despite `window_us=2000000`; its origins agree, lie within hold, and end 150µs before stop, within the 666µs service-gap budget. Reported matched identity is 998‰; exact reproduction requires the omitted coast-matching implementation.

Current remains uncertain: 257mA hold proxy is comparable to −253mA zero drift; it does not establish calibrated current headroom.

Admit only the proposed same-ELF, ≥120s OFF, physical-3A, propless/reverse, single 25%/28s screen requiring ≥14s target hold, unchanged zero-late-arm criterion, and no retry after fault. This establishes neither a causal cure for old entryTracking/25LateArm failures, a failure rate, nor new qualification.