# E734 — low-speed post-COM level revisit is insufficient

Full goal read. Prior turn progressed by identifying live stale event fault and
fresh ADC feedback. Optional bench-bemf-level-revisit adds foreground level
qualification admission below333eHz (average>=1000halfus), under active powered
owner/core/reference, input hardwareenabled/softwareunmasked, strict reference
halfinterval gate and postlevel. It pends ADC_COMP with a separate pendingflag;
real ISR still performs persistence/real timer sampling and accepted-event
guard. Accepted inputs mask the source; clear/stop revoke flag. Above threshold
helper returns without level read/wake. No synthetic commutations. This is
NOT a complete AM32 polling-mode implementation or proven parity.

Candidate lowlevel_734 installed/root SHA256:
d4468d8d1b95d6aba4a0cfc7fa746f7c904bfcaa1b895bf1595c9ebb8158b4e0.
Release-s/thinLTO/TIM16auditPASS, build/download/reset0, own guard3/18PASS.
Two same6.2%sine/6.1%drive/originallongstartup explorations, later30% requested:
- phase60: driven feedbackstale4 before release at6022us/5accepts,
  no BEMF test on this attempt; not evidence against level revisit.
- phase0: released after6accepts, BEMFcom2 then trackingstale8.
  retainedlastaccept336us/sector4,lastpoll1402,feedback1162us (age240us).
  Thus the helper did not restore post-sector5 progress on this attempt.

Both averagecause0, finaloffreadbackPASS/Uartclosed. No30%ACK/envelopegain.
Captures lowlevel_734_start62_drive61_phase60.txt and
lowlevel_734_start62_drive61_phase0.txt. Candidate remains experimental.
Next compare commanded startup speed/phase against actual rotor signal and
reference mode sequencing, not assume the miss is merely an unlatched event.
Do not remove tracking or blame hardware based on these failed firmware paths.
