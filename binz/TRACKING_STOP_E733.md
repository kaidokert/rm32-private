# E733 — retained operational tracking state, no new ISR recorder

Full goal read. E732 was progress, but lean diagnostic zeros do not identify
the live failure. Added read-only stopped_state/stopped_tracking getters for
the existing event monitor and guard; post-stop summary prints fault enum,
last accepted timestamp/sector, last guard poll and acquired feedback timestamp.
No event/ISR state writes or in-ISR capture buffers were added.

Release-s/thinLTO/TIM16helperauditPASS, build/download/OpenOCD reset0.
Frozen installed trackstop_733 SHA256:
3486f8e67534f1de56b1022cafb2ea542b161975e5fa29d7abe9d1c730a2750b.
Normal long startup, sine62/drive61/phase60, nominalcurrent3185, later30% ramp
requested. Energized4708692us, timer armed, COREOBScom2. Clean tracking8 stop.
Retained live state:
event_fault1(Stale), last_event328us, sector4, last_poll1402us,
feedback_acquired1319us; POWERPATHstop1405us. Averagecause0. Thus ADC feedback
was only83us old at the final guard poll. Producer is working; accepted-event
progress stopped after one powered sector4accept and the next COM tosector5.
This is not order/first-sector/feedback-staleness refusal. Diagnostic counters
remain intentionally zero in lean image and must not override live state.

No sustained BEMF or30%ACK. FinaloffreadbackPASS/Uartclosed. Capture
captures/trackstop_733_start62_drive61_phase60.txt. 27startup+57seed testsPASS.
Next low-speed post-COM input admission/normal polling-to-IRQ changeover,
against reference semantics; not ADC ownership debugging or flying cohorts.
Do not call the1000us deadline itself proven wrong: this run genuinely stopped
accepting. Reference does not free-run COM through arbitrary missing inputs.
Actual/root this candidate, off. Goalactive.
