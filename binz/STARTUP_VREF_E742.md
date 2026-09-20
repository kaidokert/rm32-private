# E742 — polling trial, archived trajectory, and VDDA conversion

Full goal read. E741 progressed with retained counter evidence and candidate
publication correctness fix. Installed/tested startpoll_742 FA516D9D355E64BDF51A9733405EF1015E867CB7282D0A45029BEB804A5C6A65:
guard3/18 PASS; powered62/80/0/100 direct startup, 18polls/maxcall37us,
COM1/step3/noaccept, tracking1002us/feedback777us, avgcause0. Finaloff/UARTclosed,
no target300ACK. Publication race fix did not restore BEMF.

Archived explore_696_ramp200_30s.txt differs from recent tests: run50,
terminal ehz60..200 from3.2..4.4s, phase60/drive61/initialBEMF70. Its13
accepted startup sectors were consecutive3,4,5,6,1,2,..., with measuredseed1539.
This is direct evidence of working entry; current sparse bootstrap is not
equivalent. Restored edge-only handoff (no startup-polling or level-revisit)
and old terminal trajectory while keeping new DMA/average protections.

Actual board oldramp_742b C6E98EA9F451575026D2E247520E76FCC14E4DB07ED65427AC5BD3EE67A4F007,
release-s/thinLTO/TIM16audit/build/download/reset0/guard3/18PASS.
oldramp_742b_start62_drive61_phase60_bemf70.txt stopped during sine at1929537us:
averagecause3/residual6620>6371, before handoff. Finaloff/UARTclosed/no300ACK.
Thus this run does not adjudicate the later trajectory/handoff differences.

Saved101ADC rows, vcal1662, factory-VREF conversion3000*vcal/raw yields
VDDA3301.987..3315.160mV,last3312.957. Under nominalgain10/shunt7mOhm,
residual6620/100scans corresponds764.920mA at that VDDA, not800mA. Not a
calibrated current claim: channel gain, offset and aperture remain unverified.
The old3600mV conservative assumption lowered the effective nominal current
threshold; distinguish that intentional conservative margin from target800.

Candidate avgnominal now samples VREF only when ENABLElow/outputsdisabled,
ceil(3000*vcal/vref) for conservative integer rounding, admits2700..3600mV,
sets threshold from same nominal800mA/gain/shunt and fixed SCANS. No adaptive
threshold, timestamprefresh, or raw ratchet based on fault data. Zero/invalid
VREF refuses. Conversion happens idle, not ISR. Host validates exact formula
and metadata vdda_mv; old firmware ACK is now incompatible and safely refuses.
Pure tests32/32 PASS, exhaustive VDDA2700..3600 rounding and bounds included.
Initial Rust cast syntax build failed; fixed, no stale flash after failure.

Candidate root0707B885D7D5A24F7C5E2988C77415D4B929220CA5B9980EC2B1AD35DB95D9F7,
release-s/thinLTO build0/TIM16helperauditPASS, NOT installed. Current CSA/shunt
calibration and uncertainty bounding remain unfinished; nominal conversion
does not settle gain/sign/bias or true PSU current. Next test corrected VDDA
conversion with old trajectory, then isolate measured-seed versus commanded
bootstrap if startup reaches handoff. Goal remains fully active.
