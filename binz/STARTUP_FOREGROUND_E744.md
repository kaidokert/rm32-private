# E744 — lean foreground trial and fixed lateness margin

Full goal read. E743 progressed with measured handoff and lean foreground edit.
Actual installed leanfg_7444877be755bbeb90d384d041917bc1e17b0cab3235a7d12dfe83d3c5a144b8dc7:
checked download/OpenOCDreset0, disabledguard3/18PASS. Current same-epoch DMA
feedback producer and original-age handoff token retained. No foreground
per-frame diagnosticrows/stat updates/capacitystop; scan/age stats0 intentional.

Two terminal trials sameimage/sine62/drive100/BEMF100/direct200/target300:
- phase60:40ms timeout2,48commands/40accepts,no measured seed. Handler33us,
  sector33us/late33us, avgcause0. Completed commanded sweep without late stop,
  not a causal timing-gain or sustainedBEMF claim from one trial.
- phase0:forced reason15 at21587us,25commands/19accepts, handler33us,
  retainedlate_max42us (does not include the failing sample), avgcause0.
  No measuredhandoff/target300ACK. Both finaloffverified/UARTclosed.

Source identified reason15 entry stop `late>50us`, a fixed margin, not whole
sector deadline. Goal says unsupported margins report-only. Candidate keeps
legacy50usstop outside startupADC; timed practical startup retainslate_max,
ideal original deadline/phase sequence, and stops if late>=nextplannedsector
delay before writing a role. Thus no catch-up with expired whole-sector roles.
Current/bus/nFAULT/tickgap/ISR50usoverrun/campaign/seed checks untouched.
This changes timing refusal semantics explicitly, not calibration or safety
certificate. Old failing runs remain failures. Candidate must be verified
before deployed; no result yet for that new deadline policy.
Candidate root5412dbdf9095503716a3b2e1ed5f9527c1ee0b1d69ac7d83b9a55c402005ef52:
release-s/thinLTO build0/TIM16helperauditPASS, NOT installed. No new WCET or
deadline-policy qualification claim from build/audit alone.
