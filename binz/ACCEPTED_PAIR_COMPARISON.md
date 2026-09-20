# E451 — selected fault-pair comparison

Read-only retained evidence, no powered attempt or firmware change.
scripts/drv_fault_pair.py validates capture CRC/timeline/off through existing
parsers, then compares two adjacent reference-counter cycles with the local
recorder-tail median. They have different timestamp brackets; differences
are descriptive, not an independent rotor estimate or ISR latency.

| Capture/build | Previous minus local median us | Refused minus local median us | Pair mean minus median us | Guard minus reference us |
| --- | ---: | ---: | ---: | ---: |
|range350 hold73 /7F4B|79.5|-76.5|1.5|3.5|
|timelinediv recovery72 /7C37|35.5|-110|-37.25|3|
|timelineinline recovery72 /ACEE|105|-122.5|-8.75|3|

All three are step3 refusals, but one is acceleration at197ms/73 while the
others are sustained72 at24s. These selected failures cannot establish sector
incidence or a failure rate. Existing older failures include other steps.
Near cancellation in two cases supports considering crossing displacement;
the outlined case contradicts treating every fault as exact two-cycle
redistribution. Do not average away the guard or call all three identical.

Guard/reference closure3..3.5us is far smaller than the local cycle deficit.
The large excursion is present in the accepted reference intervals, not just
the late guard/recorder bracket. This does not exclude pre-acceptance masking,
persistence rejection, comparator mux effects or rotor motion. No raw-edge
time exists in these captures to separate those mechanisms. Integer carrier
multiples in selected paired differences would not uniquely prove preemption.

ForACEE the per-sector current-minus-prior interval changes inus are
step4:-111,5:+3.5,6:+38,1:-19.5,2:+54,3:-192.5. Their sum=-227.5us equals
2839-3066.5. This is a consistency identity, not six independent observations.

Two new tests pass: all three exact failures/conservation/off checks and
rejection of a passing capture as a fault sample. Reports include capture
SHA256; raw files are unchanged. No failure reclassified as a pass.

Next discriminating instrument needs pre-acceptance evidence: qualification
attempt entry/exit and interrupt interference tied to the SAME edge and
preceding cycle, not more aggregate IRQ maxima. Reuse existing decision/
overlap instrumentation lessons; full observers previously failed CPU gates.
Design a bounded small record or sparse latch, prove timing overhead with
disabled checks before any motor run. CurrentACEE/off,350guards unchanged.
