Raw capture reports disabled preflight values and a `DIODESELFTEST ... PASS` line. It contains no per-sector samples, timing measurements, image identity, or powered-run evidence. The self-test line is a hardcoded response to `p`; conditional on this source matching the installed executable, reaching that loop implies the boot checks passed.

The source checks sampled sector states after settling, not transitions. COMG transfers roles before CCRs transfer at the next native update; this witness does not establish safety during that intervening interval. ENABLE is checked at entry and after each sector, not continuously.

The archive identity, 376 tests, clippy, instruction counts, mask duration, startup duty, command timing, retained guards, and physical setup are asserted here, not independently demonstrated. The audit reports clean roots but does not establish guard functionality or explain the unusually small reachable counts.

**Concrete eligibility blocker:** the supplied evidence does not establish safe role/CCR transitions with source complementary output disabled. Low duty and existing stops do not resolve that switching question. Resolve it on the exact executable before powered screening.
