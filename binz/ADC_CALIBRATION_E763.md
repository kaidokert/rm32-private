# E763 — PSU anchor and interrupt arithmetic audit

## Rough current/bus calibration anchor

During the 15% powered hold the operator read **230 mA at 11.8 V** on the
bench PSU, with no current limiting.  The firmware's final signed 10.05 ms
current block was approximately **255 mA** using the nominal 10 V/V CSA gain
and 7 mOhm shunts.  Thus the board estimate read about 1.11 times the supply
display at this operating point (or the PSU value was about 0.90 times the
board estimate).

This is sufficient for practical protection and envelope decisions:

- treat current and bus telemetry as rough engineering measurements;
- retain roughly 10–15% scale uncertainty, plus the distinction between DC
  supply current and sampled bridge-return current;
- the present 1 A board threshold is conservative relative to this anchor;
- do not request a PSU observation for every routine run.

It is one loaded operating point, not a linearity, temperature, phase-gain or
traceable calibration certificate. ADC rails, nFAULT, bus sag and tracking
remain independent protections.

## M0 arithmetic audit correction

The old ELF audit could forbid helpers only in an exact emitted function. It
therefore passed TIM16 while missing helpers called one or more Rust functions
below another ISR. `scripts/drv_math_audit.py` now builds the emitted direct
call graph and supports repeatable `--forbid-reachable-from` roots. Its unit
test proves a nested ISR→feedback→convert→`__aeabi_uidiv` chain is found.

Applied to frozen lean qualification image B216931B, the new audit finds:

- `DMA1_CHANNEL1` reaches `powered_timer::convert` and its software divide;
- `ADC_COMP` can reach `driven_guard::Guard::authorize` division on a startup
  path included in the image;
- `TIM6_DAC_LPTIM1` can reach two `pwm_sine` divisions on the driven-startup
  path;
- `TIM16` reaches no forbidden arithmetic helper.

This is direct-call reachability, not proof every branch executes during the
steady BEMF state; indirect calls and inlined wide arithmetic still require
manual inspection. It does prove the former “TIM16 clean” result was too
narrow to certify all interrupt work. The next optimization target is raw,
division-free DMA guard comparison, followed by startup ISR arithmetic.

No firmware was built, flashed or run for E763. The installed board remains
the frozen E762 lean image, last verified disabled with UART closed.
