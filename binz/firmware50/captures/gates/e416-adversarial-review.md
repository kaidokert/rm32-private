Raw: four tests passed; 372 were filtered out. Complementary plans produced 10 intermediate-low and two dual-high observations; diode plans produced zero and two. The incremental-cache warning did not prevent execution. No hardware evidence was collected.

The interpretation correctly limits causality, but the model assumes away critical questions: PWM level stays fixed across writes; deadtime, driver delay, current direction, and actual preload/COM behavior are absent. The shoot-through assertion follows directly from mutually exclusive Boolean definitions. Likewise, `active = preload` assumes atomic transfer rather than verifying hardware configuration or premature COM events.

Two high-side requests are not themselves proof of destructive conduction. Their persistence does not erase diode mode’s elimination of the modeled unintended lows or exclude changed recirculation/BEMF behavior.

Withholding powered admission is defensible; rejecting diode A/B is premature. Require verified transition behavior, then a bounded comparison retaining existing guards and the PSU cap, with gate/phase timing and current evidence. Diode losses and reverse recovery remain unassessed.
