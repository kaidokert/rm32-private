Raw: 117,343 accepts/commutations, zero forced, but stop reason 8 with `track_fault=1` means **Stale**, not TooFast. Equal totals cannot exclude a transient COM delay. Tail rate is `17386/0.976687 ≈ 17,801 accepts/s`; no minus-one correction. Coast supports similar rotational speed, not correct powered commutation phase.

**Use OrderRing as the next diagnostic, conditionally.** Match acceptance and bridge records by ID, unwrap timestamps, and retain bridge-bracket uncertainty. A delayed/absent bridge following an acceptance supports a COM-path problem; timely bridges followed by an acceptance hiatus supports an accepted-event gap. Inspect both: delayed COM could cause the subsequent gap. Unequal ring boundaries alone are not missing COM.

It cannot establish physical crossing timing, rejected-edge history, rotor phase, or whether an acceptance gap reflects absent edges, persistence rejection, interrupt latency, or estimator behavior. Accepted timestamps are service-entry stamps; bridge brackets are not physical switching captures. Freeze timing and terminal coverage must be verified; logging may perturb timing.

Thus it can discriminate software sequences sufficiently to **target** the next fix, not certify its cause. Proceed only after the stated build/disassembly/audit and bounded low-point screen, with guards unchanged.
