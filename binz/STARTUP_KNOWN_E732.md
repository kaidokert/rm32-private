# E732 — known startup settings and timing control

Full goal read. E731 was progress. Corrected its diagnostic interpretation:
lean feedback_inner omits FEEDBACK_N/FIRST_DELIVERY; accepted stats/tail are
also omitted. Zero values do not establish no fresh feedback or no accepted
events. POWERPATH tracking8 is real, reconstructed summary stale is not proof.

Fast known6.2%sine/6.1%drive/60degree test: timer armed age252halfus,
remaining165/arm7us, tracking8at1805us, total530764us, averagecause0,
finaloffPASS/UARTclosed. Capture startknown_732_start62_drive61_phase60.txt.

Disabled only startup-fast, restoring20/980/2000/2000ms stages/handoff4700ms.
Retained continuousADC/average/bootstrap/E731 fixes. Release-s/thinLTO/
TIM16auditPASS, download/reset0. Installed frozen normalstart_732 SHA256:
9f482e7ce5d25bbab7e35eb4a0c3416794ab160997a765ca777dd8803c754049.
Same knownsettings: total4708618us, armedage212ticks/remaining205/arm7us,
tracking8at1405us, COREOBScom2, averagecause0, finaloffPASS/UARTclosed.
Capture normalstart_732_start62_drive61_phase60.txt. Neither target ACK'd.

Longer startup did not restore running; no producerfailure established. Next
normal startup tracking/changeover/restart, not phasegrind or hardware blame.
Current event age1000us vs nominal833us at200eHz leaves167us lateaccept slack;
that arithmetic does not prove this stop false or justify removing tracking.

Added duty_split to actual standalone startup tests, preserving default62 and
testing timed100 limit/request rejection;27testsPASS. No production change
after build beyond tests in cfg(test). Actual/root normalstart_732 off.
Commanded bootstrap source remains explicitly marked STARTUPBOOTSTRAP, not
measured flying seed despite old CORESEED wording. Goal active.
