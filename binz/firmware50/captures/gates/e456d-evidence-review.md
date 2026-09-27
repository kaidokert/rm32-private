The correction is supported at source level: authority sampling, clock/origin reconstruction, live admission, budget reservation, pend, and timer update share one interrupt-masked transaction. Generation mismatch retires the schedule without overwriting COMP publication; phases outside 0/4 return before timer replacement. Reporting follows command handling and checks inactive owners, ENABLE low, and MOE clear.

The supplied run does **not** validate the corrected implementation: nothing binds its ELF hash to this source, and no `TIMEDRECHECK` counters appear. It records reason=8, stale age 243 µs against 240 µs, zero hold time, and zero forced commutations—not a cure or powered-admission result.

Remaining obligations are concrete: measure/audit the complete masked path’s latency and establish the tracking assumptions needed to disambiguate the 16-bit acceptance timestamp. The observed 110 µs guard gap cannot establish a general blackout bound.

The author’s four-root audit and 402-test claims lack supporting output here. Verdict: source-level correction supported; runtime validation and powered admission remain unestablished.
