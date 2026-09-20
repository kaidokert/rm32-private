# E740 — actual startup sequence and polling handoff

Full goal read. E739 fixed averaging coverage; still no sustained running.
Decoded its DI85: epochs1,2,4,5,7,8 / steps6,1,3,4,6,1. Bootstrap uses
commanded1666 after six cumulative acceptances, not six consecutive sectors.
Thus source metadata assumed1 is material: this is not measured rotor lock.

Existing E739c test62sine/80drive/0deg/initial100BEMF:
start10k_740_start62_drive80_phase0_bemf100.txt. Avgcause0, then COM1/step6,
no powered acceptance, trackingStale at1002us with feedback749us. Finaloff
verified/UARTclosed/no target300ACK. Phase/duty tweak did not fix handoff.

Added optional bench-startup-polling, dependent on startup-bootstrap. Only
initial commanded startup selects existing reference polling_bemf_check on
TIM7/50us. Bootstrap timer remains one-shot; no missing-event free-run.
After arm, masks COMP and initializes old_routine/zcfound/bemf_counter.
Reference polling qualification increments zero_crosses; only that completed
qualification publishes accepted progress to operational watchdog, with the
pre-commutation sector. Shared reference polling does not emit EV_ACC.
Defaults and genuine measured flying paths unchanged. No fabricated edges.
Full polling-startup parity and diagnostic-free qualification remain unproven;
reference polling still has its existing ZC trace and bounded spin wait.

Candidate/actual/root startpoll_740 SHA256
63b1cf9fcd2be83529e1f09a116a9fde3166fb74c53134b55eaf7652fa98f4bd,
release-s/thinLTO/build/download/OpenOCDreset0, TIM16helperauditPASS,
disabled guard3/18 PASS. No bench-bemf-level-revisit in this build.
Shared reference polling_bemf_check host tests3/3 PASS; these cover the
reference qualification gates/counts, not complete MCU adapter parity.

Powered startpoll_740_start62_drive80_phase0_bemf100.txt:
polling1/timer1, 19real polls/maxcall16us/maxgap82us; COM1/step5,
zero powered accepted events, trackingStale at1002us/stop1005us,
feedback918us (84us old), avgcause0/residual-2662. Initial commandedseed
1666/age266/ARR151/arm7. Target300ACKFalse. Finaloffverified/UARTclosed.
Polling did not remove the refusal, so absence of a raw edge alone is not
an adequate explanation. Period50us matches BEMF carrier50us; phase-locked
polling coverage is a limitation of this particular test, not universal
proof that all polling strategies fail. Not sustained lock qualification.

Next inspect comparator level/qualification state at the real tracking
refusal and startup-to-rotor alignment, rather than repeat blind duty tweaks.
