# E730 — normal startup bootstrap, not flying-seed certification

Read full goal attachment. E729 was progress but level revisit alone did not
restore entry. Inspected reference am32_control::zcfoundroutine: blend running
interval, commutate, increment zero_crosses, change to IRQ when ci is below
the mode threshold. It does not require this adapter's six contiguous externally
certified interval/cycle window. This experiment is NOT a full reference startup
port or verified semantic parity.

Added optional bench-startup-bootstrap (implies level revisit). On a real
reference persistence-qualified acceptance, after at least six accepted events
and commanded epoch>=6, valid step, <=20us bracket and interval667..2000,
create initial Seed using commanded200eHz interval1666 half-us ticks and the
actual current acceptance timestamp. Missing epochs are not averaged into this
estimate; it is explicitly commanded, not measured. Existing strict measured
qualification still runs and reports separately, default seed path unchanged.
Existing electrical, transfer freshness, tracking and watchdog guards remain.
Bootstrap result is consumed through the existing serialized transfer path.

Pure tests57PASS, including new admission/rejection cases. Release-s/thinLTO
build and TIM16 helper gate passed. Installed frozen bootstrap_730 ELF SHA256:
e68893f86bf002e58f1aee4d407c431c030d266ede2b4329b6b924bf169de7dc
Download/OpenOCD reset exited0; own disabled guard3/18 passed.

One directfast8%/8% startup phase0, later30% target requested:
startup released reason22 at5181us, six accepts, six commanded changes,
handlermax32us/ratepeak17. AVGDIAG cause0. Totalenergized525280us.
DRIVEX result2/power_reason0: powered transfer refused, no BEMF running or
target ACK. Existing measured DS85 seed is not the bootstrap seed; bootstrap
metadata explicitly says commanded_interval1666, measured_seed0, lock_proven0.
Capture captures/bootstrap_730_start80_phase0.txt. Final-off readback passed,
UART closed. This shows removal of strict contiguous-seed gating permits
release, not that the initial speed estimate is accurate or lock established.

Next isolate powered transfer admission/setup refusal. Do not rerun six-edge
certification cohorts or claim envelope restoration. Current experimental image
is installed and off. No current threshold change.
