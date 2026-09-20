# E817 bus-assisted adaptive 3.5 A diagnostic candidate

Staged only at freeze time. Exact E804 diagnostic image except for corroborated
foldback actuation:

- ELF SHA256: `D1C53FEF65B862F12976B4785FE076C11A9675B0F50F7A62252839EB45B06ED3`
- release profile: opt-level=s, thin LTO, codegen-units=1
- size: text121788,data1180,bss23168
- current-only sequential-ADC outliers remain subject to the unchanged two-
  consecutive-block terminal stop, but do not command a foldback
- a current block may command severity-scaled foldback only with same-block
  low-bus corroboration
- three samples below the existing8.4V code threshold command an early5%
  reduction; the unchanged50-sample bus average still owns terminal bus stop
- all other thresholds, nFAULT, tracking and watchdog behavior unchanged
-34 module-policy tests per feature cohort PASS; four motor ISR-root M0
  arithmetic reachability audit PASS

Evidence basis: E810/E811 Driver7 runs retained9/13 low-bus samples; clean E809
40% and false-fold E816 retained zero.
