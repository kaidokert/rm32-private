# Comparator capture audit - E406, 2026-09-14

Offline only. Installed044F and last verified-off E404 remain unchanged.

The existing COMPPATH instrument samples the first actual interval-count read
and classifies the completed reference call. It is aggregate, not per-event
history. E299 comppaths_hold62_01 had145198 dispatched calls:21526closed,
106905open-no-accept,16767accepted, with no unknowns. Thus most visits on THAT
279eHz build were not gate waits. This is not a current-build distribution
or an exclusive CPU-cost split; instrumentation and atomic backend changed.

Reference open-gate branch clears pending BEFORE persistence. If a live level
read rejects, there is no accepted event; later pending events can dispatch
another call. Closed-gate post-crossing pending is deliberately retained.
Neither behavior is evidence of a hardware fault. A pre-gate deferral alone
does not address the historically dominant post-gate nonaccepting visits.

The existing I85 tail has full-width handler timestamps, gate count, first/last
read, count and outcome. It does NOT timestamp physical IRQ arrival. Trace1
changes per-read bookkeeping, bypasses the static protected path, and retains
only24/32 visits. It cannot be silently enabled as an otherwise matched A/B
for current044F. COMPPATH has no rejection-index or arrival history either.

Added offline `drv_irq_trace.py --rejection-cadence <capture>`: only adjacent
sequence numbers, same sector, both persistence rejections, full32-bit time.
No bridging omitted dispatches, sector changes or accepted visits; sequence
wrap is allowed. These are handler-entry gaps, NOT physical edge intervals.

Both passing/failing archived pairs irqtail_hold54/55_01 and
cachedcomp_hold54/55_01 contain100us gaps along with other spacings. Therefore
the presence of100us rejection cadence alone is not a fault discriminator.
These older traces do not establish a current24kHz quantization result.

Next instrumentation must preserve the static comparator read implementation
and separately identify actual gate/reject/accept outcomes. Reusing trace1
as-is would change the mechanism being tested. Prefer a bounded optional
decision-level record after each reference call, recording its actual first
counter read and accepted/stopped outcome, without per-level work or another
full-IRQ chronology. Its overhead still needs codegen and disabled timing
qualification before powered use; no new instrumentation is installed here.
