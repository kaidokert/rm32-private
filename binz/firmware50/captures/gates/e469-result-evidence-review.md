Raw evidence: E468 records six accepts, zero hold, and a stale age of 1028µs against a 1000µs limit. E469 records 20,278ms hold, 261,779 hold accepts, zero forced events, and coast_ehz=2165. Flash verification matches E469’s full SHA256; postflight reports outputs off and nFAULT=1.

Arithmetic checks:

- 199/1333 = 14.9287% applied duty.
- 261779/20.278 = 12,909.507 accepts/s; dividing by six gives 2151.585Hz, conditional on six valid accepts per electrical cycle.
- Sector counts sum to 253,883; adding 30,772 coalesced accepts gives 284,655.

The evidence supports one successful nominal-15% baseline hold and one candidate tracking failure. Keeping the candidate unpromoted and avoiding escalation is justified; cadence causation and envelope improvement are unproven.

“Matched997” is reported, but its calculation is unspecified. “Normal deadline,” “foldback0,” and the tail’s “by design” explanation require definitions beyond these records. UART closure, current-source state, predeclared exposure limits, and cadence-dependent filtering are not independently established here. Preserving old cadence is a reasonable experimental constraint, not a demonstrated causal remedy.
