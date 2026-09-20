# E850 fixed-advance-20 exploration image

Exact E846 adaptive 3.5 A exploration/protection composition, replacing the
diagnostic scheduled 18/20/22 policy with fixed advance level 20. This retires
the known over-advanced level 22 above 35% while preserving the best timing
point observed in E836. It omits the accepted-event interval ring, nFAULT
classifier, and CPU accounting.

- ELF SHA256: `0A5F301A057ADA764CED76DB355B2CB8C8EF6BFDF0142719759A7AEE09470FC4`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121448, data1180, bss23156
- four motor ISR-root M0 arithmetic reachability audits PASS
- real signed-current/bus/nFAULT/tracking/deadline/watchdog protections remain
  active; the known single-cycle timing metric remains report-only
- exploration only; a separate lean fixed-20 build is required before any
  operating point is called qualified

Source audit before this build also falsified comparator hardware blanking as
an AM32 parity feature: the cached G071 AM32 tree initializes COMP blanking to
NONE. Its `comp_pwm` setting controls complementary bridge PWM.
