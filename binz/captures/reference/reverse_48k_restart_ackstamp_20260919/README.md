# Reverse 48 kHz: restart duty ownership + 50% ACK timestamp

Flashed/verified on explicit G071 SN066CFF343433464757233430, device0x460.
SHA256 `72FF21BB57709F9CC332FC2D36F551762D51659CA7F27118891E98C4894C0327`.
`release-hybrid` is release-s/thin LTO/codegen1 for binz and z for dependencies.
Size: text 125372, data 1200, BSS 17144 bytes. Same protected reverse48k
feature closure as the cache-owner image, plus `bench-duty50-ack-stamp`.

The CPU diagnostic run exposed a restart bypass: after a tracking-loss ordinary
restart, a live upward `du` command went straight to the guarded PWM writer,
skipping the intended 5%-per-2s restore schedule. This image defers upward
requests into `normal_restart::Resume`, retains its cadence and current rung,
and ACKs the **actual** published duty. Lower requests remain immediate.
The pure policy harness passed 7/7; four ISR-root soft-arithmetic audits passed.
The prior CPU diagnostic result did not characterize sustained CPU load; it
stopped tracking near 40%, then fast-sagged during the bypassed restart.

Foreground-only `LIVEACK50` prints the powered-segment timestamp at which
the successfully guarded duty500 request was ACK-queued, plus the terminal
stop timestamp and their difference. The PWM transfer may follow by up to one
carrier period. This does not change ISR work or electrical guards. Compare
`age_us`, not total powered runtime, across 50% runs; two earlier captures
lacked the ACK timestamp, so their 63 ms stop-time difference was suggestive
only. Do not treat this diagnostic as lean qualification.

Disabled guard3/18, roledu1006/6, duty6/6 and p/i off/nFAULT high passed.
Powered evidence, all retained under `captures/reverse_direction_2026-09-19_restart_ackstamp_*.txt`:

- Two first 10% gates used BEMF duty70 and refused handoff before powered
  operation; a direct-start recovery gate with BEMF duty100 also refused
  initial handoff. These are not 10% powered failures.
- Legacy host 50→200 eHz startup with BEMF duty100 reached a protected
  10%/15s hold: POWERPATH reason2, zero foldback/bus-low/phase-rail/tracking
  stops, final outputs off. The host fixture falsely timed out looking for
  `COAST END`; compact mode ends with `DONE`. The fixture was corrected.
- A 10% injected Tracking8 restart stopped the first powered segment at5s,
  waited one second disabled, then the second startup refused BEMF acquisition.
  It did **not** exercise upward-request deferral; the source policy is
  host-tested but powered-unverified. Outputs off.
- Two matched ACK-paced 10→50% ramps reached duty500 and stopped on fastbus
  reason26. `LIVEACK50 age_us` was131851 and155129;
  POWERPATH stop_us7463583/7533566. Both had zero current foldback,
  tracking or nFAULT stop and final outputs off. Phase-rail count was3 in
  the first,0 in the second; a rail is not necessary for sag. This is a
  ~0.13–0.16s post-step signature in **this** ramp, not proof of a fixed time
  constant or of the physical source. 50% remains unqualified.
- A separate 1%-per-0.5s climb also ACKed50, then fastbus reason26 after
  167307us (ACK23641419, stop23808726), with zero foldback/tracking/nFAULT.
  Thus a single5% final jump is not necessary. All three stamped runs stop
  0.132–0.167s after entering50; different climb durations make a slow
  wall-clock buildup less plausible, but the dataset still cannot order
  commutation/current and bus onset. Exact transcript:
  `captures/reverse_direction_2026-09-19_restart_ackstamp_50pct_smooth1.txt`.
