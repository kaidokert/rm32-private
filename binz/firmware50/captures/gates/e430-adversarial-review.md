The packet supports only a limited 15% observation, not clearance toward 80%.

- **Attribution:** One run per different ELF cannot isolate constant scheduling cost. Coast estimates are similar, but 12 versus 13 µs processing maxima do not establish improvement; COM lateness increases from 5 to 14 µs. Firmware audit/test claims are not independently evidenced here.
- **Observer effects:** Both captures are instrumented; identical recorder code does not establish identical timing impact. The 128-pair tail covers roughly 10 ms, not the entire hold or the earlier failure region. Zero margin counters provide no demonstrated margin coverage. At 8 MHz, two ticks equal **250 ns**, not 125 ns; the bracket does not bound total instrumentation overhead.
- **Safety:** OFF snapshots establish endpoint state only. Uncalibrated current, −111 mA zero drift, absent thermal readings, and unexplained `ma_allow=31857` prevent treating the current telemetry or 120-second rest as validated protection.
- **Discrimination:** A bounded 25% screen could establish whether this candidate completes that specific exposure without the prior LateArm failure. A pass cannot attribute improvement to scheduling cost; a failure needs onset evidence, not merely the final tail.

Conditional next-test support requires explicit abort thresholds, independent supply-limit verification, and retained failure-onset timing. Those are not established by this packet.
