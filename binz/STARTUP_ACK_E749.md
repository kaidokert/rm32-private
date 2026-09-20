# E749: reject false startup-ramp success; valid softer direct startup

Goal read. Previous turn made implementation/evidence progress but its
interpretation of early host frequency ramps was wrong. No firmware flash
this turn; actual remains practical_748 E36C2BA93CFC4F7F86BFBC0B76DB8738633DAF73CFE5A8C4950AC418866BB09D.

Source shell-pwm live ehz branch permits steps ONLY after
ALIGN20 + START980 + RAMP2000 control ticks, and change<=10Hz.
E748b/c captures contain !step for early commands. These weren't the requested
earlier/slower frequency ramps, despite fixture exit0. Corrected E748 memo.
Actual source starts100Hz, not50; run50 ramps DOWN to50 after the catch dwell.

drv_capture now requires each frequency command's exact F<target> ACK before
sending the next. !step/!hz/!busy immediately reject; missing ACK times out
after0.5s while still driving. Partial data and final-off cleanup retained.
Three startup-ramp tests PASS (timing validation, exact/partial/wrong ACK,
explicit refusals); eight capture tests PASS. No motor test of this new
ACK branch yet: actual tests below use no host startup frequency steps.

Valid firmware direct100->200, catch40->target62, phase60, BEMF70:

- captures/practical_749_direct_catch40.txt, forced61: sine reaches acquisition
  without average stop; forcedwindow40ms/48commands/32accepts times out before
  measuredseed. Handler29us/late40us/averagecause0. No powered handoff.
- captures/practical_749b_direct_catch40_drive100.txt, forced100: acquisition
  completes5744us/7commands/8accepts, measuredseed1558/age270/arm7us;
  two poweredcommits, latestaccepted279us/sector4, tracking stop1305us,
  feedback1026us fresh, averagecause0/residual2721<6931.

Both final-off verified/UART closed/no300 ACK. Softer direct duty ramp avoids
the nominal average trip in these two runs, but doesn't solve sustained
handoff. This is a startup/control problem, not demonstrated supply/hardware
ceiling. No calibrated-current or CPU claim. Current-model review found
same-epoch raw zero and signed channel means, not proof of physical calibration.
Keep electrical protections; normal-startup restart and sustained lock remain
unfinished. Do not repeat early ehz experiments against this firmware as if
they alter its startup trajectory.
