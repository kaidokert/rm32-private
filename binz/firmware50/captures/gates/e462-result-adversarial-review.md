Raw arithmetic: `wait=(59>>1)-floor(59×16/64)=29−14=15µs`; spend17 exhausts it by2µs. Hold rate `11782/0.936≈12587.6/s`; tail `11781/0.936750≈12576.4/s`, or2096.1eHz. Against coast2142eHz, that is978.6‰, compatible with reported980‰ but not independently reproducing its calculation. `1e6/(6×79)=2109.7eHz` uses the rounded interval. Hold covers936ms; the tail does not establish a full two-second window.

The source supports LateArm shutdown and an unmapped guard15 becoming UnknownGuard; numeric28 still requires the omitted enum mapping. Postflight shows an off state, not shutdown latency. Zero margin histograms provide no distribution evidence; current drift285mA exceeds the237mA hold proxy.

The proposed preemptor needs qualification: default COMP and COM are priority peers, so COM cannot interrupt an executing COMP. Recheck can delay COMP entry, but this spend starts after entry. With `com-top`, interruption becomes possible. Maxima and boot-cumulative recheck counts cannot establish causality or successful recheck acceptances.

Smallest discriminating change: add a source/configuration test pinning the experimental binary’s actual COMP/COM priorities and timed-recheck selection. Resolve that prerequisite before timing instrumentation. Separately regression-test reason decoding; no diagnostic rebuild solely for labeling. No repeat or duty increase is supported.
