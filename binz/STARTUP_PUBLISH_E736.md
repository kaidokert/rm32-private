# E736 — startup guard feedback comes from the DMA producer

Full goal read. Prior E735 progressed by fixing carrier compare. Retained
DA85 frames stamped201us apart then stopped foreground delivery before driven
feedbackstale. Added fault-only ADC hardware snapshot afterbridgeclear.
Initial736 incorrectly captured afterENABLElow, which invokes ADCquiesce:
timerCR10/NVICdisabled are shutdown consequences, not evidence of fault cause.
Corrected736b to capture aftergate/MOEclear but beforeENABLElow and reset
snapshot eachrun. Its attempt did not exercise reason4, so new snapshot branch
remains unverified. It enteredBEMF then trackingstopped, not snapshot-qualified.

Source inspection found startup guard feedback published only by foreground
cache reads while ADCproducer already validates/publishes fresh frames. Fixed
startup DMA branch to call driven_run::stream_feedback once per real completed
frame while drivenowneractive. Uses sameconvertedFeedback and original acquisition
stamp mapped with oneTIM17snapshot/ACQ_CLOCK; no new averaged sample/cache read
or timestamprefresh. Existingaverage remains once-perDMAframe. Foreground
retains its ADCrecords/LAST_FEEDBACKtoken but no longer replays older frames into
startup guard. Guard start/stop/freshness checks unchanged. This is a production
publication fix, not proof of cause of every prior startup-stale refusal.

Final startpub_736c installed/root SHA256:
18fa024e4292fd713a0aa643cd61ca8ff6f837c030e0d50a8040b389f0603eb7.
All3candidate builds release-s/thinLTO/TIM16auditPASS/download/reset0.
Final test long62sine/80drive/0degrees: startuprelease22at8862us/6qualified
accepts/11changes, foregroundage_max569us, no startupfreshnessfault. BEMF
timerarmed then trackingstale8at1005us: no poweredaccept (liveeventsector0),
feedbackacquired782us atlastpoll1002us (220usold), COREOBScom1/physicalstep5.
Averagecause0. No sustainedBEMF/30%ACK, finalUARToffPASS/sessionclosed.

Captures adcstop_736_start62_drive80_phase0.txt (snapshotorderinvalid),
adcstop_736b_start62_drive80_phase0.txt (no snapshotbranch),
startpub_736c_start62_drive80_phase0.txt. Next sector5/phaseA post-COM comparator
configuration versus reference path. Actual producerworking, nothardwarewall.
Faultsnapshot notretired/qualified; remove from zero-diagnostic qualification
build eventually. Goalactive, boardoff.
