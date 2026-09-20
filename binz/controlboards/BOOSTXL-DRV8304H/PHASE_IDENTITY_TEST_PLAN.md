# Targeted phase identity: evidence requirements

Pending campaign step, not a completed wiring verdict. Motor-unplug
confirmation is required before uncoupled output tests. PSU remains at the
operator-set 800 mA limit. No simultaneous high/low combinations should be
introduced on the assumption that their physical pairing is already correct.

## What is already established

- The accepted waveform drives rotation with all three current channels active.
- All six MCU gate pins read low with MOE/ENABLE off.
- Enabled, zero-drive IA/IB/IC all sit near midscale (Entry 014).
- Ordered coast comparator transitions show rotation, not labeled identity.

## What each next observation must establish

1. With windings disconnected, identify the voltage-feedback response to each
   individual source command. Record all three comparator inputs and C/neutral
   ADC, before/during/after bounded excitation. Select one input at a time;
   do not assume nominal INH really lands on a high input. Confirm behavior
   repeats and is above the measured baseline/noise, with no fault.
2. Comparator high means neutral above the selected phase in the present
   configuration; verify this against C-minus-neutral rather than trusting
   the label. A unique reproducible response per command establishes a
   command-to-feedback permutation. Multiple responding inputs, missing
   response, or insufficient signal are inconclusive, not a wiring diagnosis.
3. Low-input tests need a known initial voltage or bias. An output already
   sitting at ground cannot prove that a low command worked. Characterize
   floating-node discharge first; do not mistake passive decay for a low-FET
   response. Establish which low discharges the node identified by each high,
   with break-before-make and no overlapping unproven high/low commands.
4. No winding current means no ISEN identity proof. After gate pairing is
   established and motor reconnection confirmed, use short controlled current
   paths in both directions, subtract enabled offsets and compare all three
   channels. Low-side shunts are observable only in appropriate conduction
   windows; do not assume every phase current is measurable at every PWM point.
5. Report relative identity separately from physical board labels. A common
   permutation of drive and feedback can be behaviorally correct yet leave
   silkscreen A/B/C unanchored. If needed, ask for one precisely named physical
   anchor check rather than demanding a full wiring audit.

Each record must include actual pin/ADC/COMP identifiers, timestamps, active
input, measured gate states, nFAULT, VBUS and before/after safe-state readback.
Write numerical signal/noise and repeatability gates before energized tests
once the no-drive and floating-node baselines are measured. Do not silently
promote this plan, an expected pattern, or a command label into measured truth.
