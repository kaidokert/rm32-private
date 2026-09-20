# E755–756 — current guard had the wrong sign

Source evidence: cached `controlboards/BOOSTXL-DRV8304H/drv8304.pdf`,
SLVSE39B page 28, equation 3 and Figure 30. The equation is
`I = (VREF/2 - VSOx)/(GCSA * RSENSE)`. Figure 30 connects SPx to the
inverting input. The board schematic places SPx above the low-side shunt,
SNx at common ground. Positive return current therefore lowers SOx.
Rendered source retained as `captures/drv_csa_page28_e755.png`.

The current measurement plan's former PLUS sign was incorrect. The average
guard implemented that incorrect sign and failed to trip on positive load.
Negative raw residuals in E753 do not indicate failed sensors or negative
load current. This supersedes E754's unresolved-polarity interpretation.

## Fix

`average_current::Limit::scan` now calculates zero minus the scan sum.
Signed regeneration remains negative; recirculation cancels, and no absolute
value is used. Baseline scaling rounds upward by less than one raw count
per block, conservatively for the corrected positive residual direction.
No gain, shunt, current target, bus threshold or waveform setting changed.

The 34 startup policy tests pass, including polarity, exact threshold,
latched fault, regeneration and cancellation. The adapter's 9 host tests
pass, including configuration refusal, producer continuity and revocation.
The new explicit equation regression test was added after the MCU build;
it changes test-only code, not the flashed policy.

## Installed exploratory image

Frozen `captures/reference/currentsign_756/shell-pwm.elf`, SHA256
`73190f5f6483eecd062f08559d16c67b830e44e6449b31f241e103712bf93f8e`.
Same feature set as rotating E753b. Release opt-s/thin LTO/codegen1 build,
direct TIM16 helper audit, download and OpenOCD reset succeeded. Own disabled
guard check passed (3 timer faults, 18 post-stop refusals, outputs off).
Operator confirmed PSU limit changed to exactly 1 A before the run.
Nominal firmware current target remains 800 mA, not measured calibration.

Capture `captures/currentsign_756_start62_bemf70_ramp30.txt`:
startup works, live 10/15/20% acknowledged. Before 25/30%, average-current
fault 25 stops at powered 4,372,052 us: residual 3541 > allowance 3468,
approximately 817 mA nominal. Peak raw deviation 635, minimum bus 10,948 mV,
10,621 applied commutations, last accepted event 4,371,951 us and fresh
feedback 4,371,782 us. No tracking or bus stop. Final off verified, UART closed.
This is not a 30% successful run, and not independent physical calibration.

E753's final 25% block residual -3670 becomes approximately +3670 under
the corrected equation, about 847 mA nominal versus operator 812 mA.
Those samples and the display are not synchronized, so agreement in scale
does not certify accuracy. CSA gain, offset drift and PWM occupancy still
need quantified verification. A same-image 15% hold is retained separately.

## 15% hold and independent anchor

`captures/currentsign_756_start62_bemf70_ramp15.txt` completed the full
30,000,005 us powered window with deadline reason 2. Live 10/15% ACK,
122,664 applied commutations, minimum bus 10,865 mV, peak raw deviation 618,
last accepted 29,999,859 us, feedback acquired 29,999,880 us. Final off
verified and UART closed. Operator reported 230 mA while it ran.
Final average block residual 1104, allowance 3468 gives approximately
254.7 mA nominal. This agrees in direction and scale, but is not an exact
calibration: the display and final block are not time-aligned, gain remains
nominal, and the aggregate includes the live ramp. No thresholds changed.
