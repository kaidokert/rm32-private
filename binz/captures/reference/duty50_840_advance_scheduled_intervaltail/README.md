# E840 scheduled-advance diagnostic

Evidence-driven timing schedule for the direct-PWM binz path:

- startup and requested duty below30%: advance18
- 30.0..34.9%: advance20
- 35.0% and above: advance22

The live-duty writer precomputes the level; the COM ISR performs one atomic
load and range check, with no division. Polling/startup remains advance18.
All other E835/E837 diagnostic and protection selections are unchanged.

- ELF SHA256: `78ADE63D90AE9B749C8A1DC59E6F1B54AF35CEE58C89752BADF8B8C37EEF4A06`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text122276, data1180, bss23680
- post-run `LIVEPARAM` reports the actual scheduled level and source rather
  than the low-duty compile-time fallback
- scheduled minz-core cohort: 86/86 PASS
- four motor ISR-root M0 arithmetic reachability audits PASS
- diagnostic-only; not envelope qualification

The initial E841 schedule used the original40% boundary and stopped at39%
with terminal advance20, so it never exercised advance22. The source boundary
was then moved to35%; the next frozen image supersedes this artifact for bench
use.
