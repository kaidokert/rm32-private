**One conditional sag-only 70% capture is proportionate as a diagnostic**, provided the checks below pass. This is an assessment, not authorization.

The same-image evidence shows one 65% deadline completion with **12.278 s at target**, and one 70% sag stop after **9.025 s at target**. Both report final outputs off and nFAULT high. Neither establishes repeatability or a loaded envelope.

Reason26 plus `streak=3/tripped=1` supports attribution to the sag guard. It does **not** identify supply limitation, thermal deterioration, current surge, reference drift, or a switching notch. Run-wide `bus_min` and terminal filtered reference cannot reconstruct the deciding comparisons. Current accumulation follows the sag verdict, so reported current peaks exclude the deciding scan; the proxies and zero drift further limit amperage conclusions.

Critical checks before the proposed test:

- Verify the new composition preserves **all staged carrier and protection settings**, boot carrier selftest, and run-key arm/disarm/dump lifecycle. Confirm the deciding row is retained before freeze and the dump occurs after safe-off. Existing `sag-capture` is not a matched carrier control.
- Check final ELF RAM and **worst nested stack headroom**, including ring initialization and interrupt frames—not merely ring size. Confirm COMP/COM machine code matches the lean image.
- Bound the recorder’s **interrupt-masked push cost**, including every-32nd slow-ring write. Identical roots do not eliminate observer effects: foreground recording masks interrupts and delays return to protection processing after a sag verdict.
- Make “50 clean” explicit: target attained, normal deadline completion, no protection/latency failure, valid complete trace, final outputs off. Any failure ends escalation; retain it. Preserve the stated OFF interval, PSU limit and guards.

The lifetime IRQ peak above64 does not contradict the supplied post40ms enforcement lifecycle. Zero late arms and disabled histograms provide no complete timing-margin proof.

`speed.py` also needs interpretive restraint: it fits **all 32 spacings**, despite documenting eight. Its powered averages span 2.278/3.025 s; steadiness near a fault is unverified. Speed residuals cannot establish terminal acceleration or crossing causality.

The proposed ring can distinguish sampled operand/reference histories. It cannot establish physical crossing order or characterize a pulse between ADC samples.
