# Hardware-PWM bench shell

## Build profiles and guarded handoff checks

The default `cargo build --release --example shell-pwm` retains normal ADC
feedback, all motor guards, compact capture/replay, and the reference observer.
Older characterization-only commands `adcwindow`, `adcsingle`, `adcsettle`,
`adcsweep`, `adcverify`, `adctiming`, `obstiming`, `obstiming200`, `corebench`,
and `coreirq` require the optional build:
`cargo build --profile bench-probes --example shell-pwm --features bench-adc-probes`.
That full characterization build is size-optimized, lives under
`target/thumbv6m-none-eabi/bench-probes/`, and does not inherit the normal
release build's measured ISR timings. Use normal release for motor campaigns.
Without that feature these commands fall through to the unknown-command reply.
This saves about 7.9 KB of flash; their source is preserved.

`guardcheck` is bridge-disabled. It tests feedback-stale, missing-event and
campaign-deadline stops, then calls the real guarded writer for all six sectors
after each stop. Each `POSTSTOP` must report `refused=6 expected=6 disabled=1`.
Synthetic feedback tests shutdown logic, not analog calibration or current limits.

`recordcheck` is idle-only and requires ENABLE/MOE/gates already off. It invokes
the actual recorder for six sectors in each of two stopped configurations.
Expect `pass=1 rejected=12 expected=12 unchanged_log_and_end=1 disabled=1`.
This checks stopped-callback rejection without enabling interrupts or granting
gate authority; it is not a test of every possible NVIC preemption instant.

`engagedry1` arms an observe-only reference handoff after the next run reaches
4.7 seconds. It acquires a measured seed while disabled, then runs real BEMF
reference processing and independent ADC/timer guards for at most20ms.
`engagedry0` cancels. `ENGAGEDRY path_result=1` means the path ran, not that it
passed; inspect POWERPATH/COASTREF/ACCEPTTIMING and decoded accepted events.
Entry118 records the first full integrated dry pass. No powered reference
output authority is provided by this command.

`engage1` (idle only) arms the experimental powered variant; `engage0` cancels.
It uses the run's duty (still capped at10%), measured seed, real reference COM
and the independent20ms guard. POWERCOMMITS counts applied gate writes;
POWERFEEDBACK reports scan count, maximum absolute current offset in raw ADC
counts, and minimum bus voltage. These are not average-current measurements.
Entry121 fixes the wake-up sequencing: gates/MOE stay off during a1.1ms wake,
nFAULT must release before acquisition, and ENABLE stays high through the
fresh-seed handoff. FLY disabled=0 is expected only on this awake acquisition;
it does not mean PWM was applied during acquisition. DRVWAKE ready=1 reports
successful wake completion, not the live ENABLE state at dump time.
The first awake attempt achieved7 gate commits before the independent timing
guard stopped it. This remains experimental, not qualified closed-loop lock.

Entry122 changes the independent guard to ordered333..1000us event gaps plus
rolling same-sector4000..6000us cycles (~167..250eHz). POWERPATH reason12 is
cycle timing; reason8 remains order/individual interval/staleness. The old
ACCEPTTIMING report still evaluates its historical666..1000us per-sector
band and is report-only; fault2 there need not be the active shutdown reason.
Neither monitor is a rotor-lock certificate.

New source command `engagedu60` selects6.0% only for the powered reference
segment, leaving normal run/catch duty unchanged. `engagedu0` restores following
the run duty (boot default). Idle-only, persistent until changed/reset, cap10%.
ENGAGEDUTY reports requested duty; POWERCOMMITS determines whether any gates
were actually driven. Entry123 verifies this command on hardware.

Powered COMP IRQ protection now uses64 calls per fixed1ms bucket, reported by
IRQRATE, not the older256-call lifetime quota retained for diagnostic modes.
An exceeded rate stops gates immediately; feedback-age and guard-tick checks
remain independent. Entry123 records the first complete powered20ms window
(6.5% catch/run, engagedu60), not repeatability or sustained lock.

Mixed-mode fixture captures add `COREHISTORY` / `H85` / `COREHISTORY END`.
Decode with `python scripts/drv_core_history.py captures/<tag>.txt`.
Each CRC-protected record contains eight u16 fields: elapsed microseconds,
reference event kind(0=commutation,6=desync),logical step1..6,event interval,
average interval,previous average,polling flag,zero-cross count. At EV_REF,
the reference has updated its sector-history slot but the foreground may not
yet have recomputed average; do not require snapshot average to equal the
new history average immediately. At EV_DSY,previous average is captured before
the reference updates it. Polling changeover occurs after that commutation
callback,so its last EV_REF can still say polling1. These are commutation/
desync timestamps,not polling zero-cross timestamps.32rows,overflow rejected
by decoder;16bit time is valid only for this bounded sub65ms test.

`coastpoll1` arms reference polling startup for the next bridge-disabled
`coastref1`..`coastref6` run (or `coastcheck`); `coastpoll0` cancels. It does not
start the motor itself. This new source mode is not yet hardware-qualified.
The reference `start_motor` advances the supplied seed sector and initializes
its interval/count to10000/5000 half-us ticks. TIM7 calls the reference polling
BEMF band at nominal20kHz, then COMP/COM take over when the reference changes
mode; polling stays available for fallback. Output HAL remains record-only.
The full reference duty/ADC safety ISR is NOT called with dummy ADC data.
DRV powered-drive guards remain unchanged, and this sensing runs only with
the bridge already disabled. The20ms deadline stops new polling invocations;
an in-progress reference blocking wait can finish past that deadline. Measure
actual duration and maximum call cost before relying on a tighter bound.
COASTPOLL reports polling commutations, transitions toIRQ, first transition
time, wait-guard hits and recovery/desync counts. ACCEPTLOG remains IRQ-only;
do not interpret it as a complete mixed-mode accepted-event timeline.

Observation wire v8 preserves raw comparator/PWM bits and voltage fields.
Only detector decisions now consume inverted reference-HAL levels consistently
with the physical drive sequence. Python exposes `reference_late_on` separately
from raw `late_on`; host replay retains the original convention for v1-v7.

`obsphase30` arms a one-shot +30 electrical-degree offset at the sine-to-six-step
observation handoff; `obsphase0` clears it. Signed range -60..60, idle only.
Positive advances commanded phase; it is not a calibrated rotor correction.
The observation consumes the setting and dumps actual OBSPHASE. If startup
fails before observation, clear it explicitly before another experiment.

`coretrace0` (idle only) disables detailed COMP IRQ rows and per-read trace
bookkeeping; reference persistence, IRQ caps and safing remain active.
`coretrace1` restores detailed tracing (boot default). CORETRACE in each
observation summary identifies the setting; IRQTRACE n=0 with tracing off
does not mean no interrupts. This setting persists until changed/reset.

`catchdu65` selects a6.5%100eHz catch, independently of target duty. Boot
default is70(7%). Fixture `--catch-duty-tenths 65` selects and restores70 on
exit. `--duration 0.9` can stop a campaign for a startup coast checkpoint;
actual stop time comes from TIMING, not the host's requested duration.

## UART DMA spike

`python scripts/drv_uart_dma.py --tag uart_dma_trial` performs driver-disabled
32x1KiB stop-and-wait roundtrips per baud. Firmware command `uartdma<baud>`
announces the change at115200, waits200ms, uses USART3 RX/TX DMA, checks CRC,
transforms payload, then restores115200. Two-second per-transfer timeout also
restores115200. Do not type other shell commands during the binary transaction.
Reset is the fallback if transport recovery fails; no drive command is issued.

Entry044:115200,460800,1M,2M,3M passed all32frames each.4M received no complete
first reply and recovered safely.6M/8M were not tested after that failure.
This is a short duplex transaction spike, not continuous streaming qualification.
Default shell baud remains115200; no assumption that MCU maximum equals link maximum.

## Variable electrical-frequency target (Entry040)

- `ehz200` or `ehz 200`: set campaign/observation target without starting.
- `ehz`: query target, duty and duty ceiling.
- `run`: start the stored target; `run100` sets100 eHz and starts.
- `du70`: independently set7% duty; ceiling remains10%.
- `obs1`: append bounded six-step observation at that target; `cap1` opts into dump.
- `off`: stop. Settings are accepted only while idle.

Target range1–250 eHz. `sf` continues to control the separate fixed-sine mode.
Campaign ramp now supports targets above and below its100 eHz catch without
unsigned underflow. Configurable range is NOT a qualified operating range:
no automatic V/f duty increase; staged current/bus/tracking evidence is required.
The capture fixture accepts `--target` through250 and `--observe` uses it.
The historical coast45–55 diagnostic remains baseline-specific, not a universal gate.

Observation stop reason6 means the independent event-freshness watchdog latched:
no new candidate for more than22.5ms (reference BEMF timeout). It is polled even
without events; duplicates or a late event cannot reset an expired deadline.
Currently covers only the bounded six-step observation, not sine startup/hold.
Candidate freshness is not proof of rotor lock or qualified higher-speed safety.

Entry042 moves RUN/SINE banner transmission before PWM enable. A control
interval exceeding2ms now stops drive with reason7 (timing overrun), using
the usual coast/dump path. The1kHz sine update rate itself is unchanged.

Timer-only diagnostic `comtiming` (Entry 036) forces gates/MOE/ENABLE off,
then probes TIM16 at 2 MHz without interrupts or pin outputs. Sixteen trials
per mode compare immediate ARR, preload without update, and explicit preload
transfer. Old ARR1999/new ARR399 measured 200–201/1000–1001/200–201 us,
respectively. TIM16 is stopped afterward; use `p` and `i` to verify safing.
This tests elapsed update timing, not COM ISR latency or motor control.

`comirq` tests the immediate-ARR G071 COM adapter through an actual TIM16
interrupt, still with gates/MOE/ENABLE off. It runs32 deadlines idle and32
with foreground ADC reads; the ISR timestamps and disables its own update
interrupt, never changing phase outputs. Entry037 measured201–202 us for
200 us requests with no missing events. TIM16/NVIC are stopped/masked after
the diagnostic. Full controller/COMP ISR load is not represented by this test.

`corebench` runs actual minz-core COMP/COM routines with synthetic comparator
levels, real TIM2 interval time and TIM16 interrupt delivery. Output HAL is
record-only: it cannot energize phases. Two32-event trains represent200/250 eHz.
Summaries include service maxima, predicted interval, decisions and trace drops.
This is an M0 execution benchmark, not physical BEMF sensing or rotor lock.

`compirq` forces COMP2 output transitions by toggling polarity while all drive
outputs are off; it tests EXTI18 rising/falling delivery on the selected phase
mux. Idle phase-vs-neutral levels can chatter, so output-level agreement is not
guaranteed. `compirqref` uses internal VREFINT on INM as the stable control;
Entry039 passed96/96 edge-flag/output checks. Both restore COMP2 CSR, mask/clear
EXTI18 and mask ADC_COMP afterward. Neither verifies external wire continuity
or motor BEMF. The diagnostic IRQ is deliberately one-shot, not AM32 service.

Use `examples/shell-pwm.rs` on the free-wired G071 / DRV8304H rig.
`shell-sine.rs` is preserved unchanged as the successful reference; it already
contained the initial raw-TIM1 conversion. The clone removes unused GPIO drive
code and manual gate/ENABLE assignments, clarifies duty units, and adds timing
diagnostics. This is open-loop drive, not closed-loop speed regulation.

## Terminal operation

COM41, 115200 baud. Send each line separately using the existing `ss` terminal:

```text
off
du60
run50
```

This selects a 6% target, then runs 20 control ticks of 1% alignment, 980 ticks
at 100 electrical Hz / 7%, a 2000-tick ramp to 50 Hz / 6%, and a hold until
the five-second limit. `du70` means 7%, not 70 microseconds. Duty is capped at
10%. Keep the PSU current limit at the operator-set 800 mA; firmware does not
configure that limit. Keep the motor secured and clear of moving parts.

`off` stops early. In the quiet-mode source update, wait for `DONE` before
another command; the default no longer emits full data. Send `cap1` while
idle to arm one completed-attempt Ascii85 dump, or `cap0` to cancel. The fixture
does this automatically and requires acknowledgement before driving. This
update is flashed and bench-qualified (Entry 013). The live fixture rejects
older firmware without the `CAPTURE armed one-shot a85-v1` acknowledgement.

When capture is armed, wait for `COAST END` before another command: gates and
ENABLE are off during the coast capture and dump. Most commands are rejected
while running/capturing. Idle checks:

```text
p
i
pwmcheck
```

`p` reports mode, ENABLE, MOE, CCRs; `i` reads physical MCU pin levels and
nFAULT. Expect mode/coast=0, all six gates=0, ENABLE=0, MOE=0, CCRs=0,
nFAULT=1. `pwmcheck` keeps
ENABLE low, temporarily generates 6% PWM, counts PA10 edges, then clears MOE
and all CCRs. Expected: 101 rising edges spanning 10000 us.

Close `ss` before host capture; only one process can own COM41:

```text
python scripts/drv_capture.py --target 50 --duty-tenths 60 --tag my_pwm_run
python scripts/drv_capture.py --infile captures/my_pwm_run.txt --tag my_pwm_replay
```

## Peripheral and capture contract

| Phase | High / complementary low, AF2 | TIM1 channel |
|---|---|---|
| A | PA10 / PB1 | CH3 / CH3N |
| B | PA9 / PB0 | CH2 / CH2N |
| C | PA8 / PA7 | CH1 / CH1N |

TIM1 runs at 64 MHz, PSC=0, ARR=6399: 10 kHz edge-aligned PWM.
CCMR1=0x6868 and CCMR2=0x68 select PWM1 with preload; CCER=0x555 enables
active-high main/complementary pairs. BDTR=0x0c1a with MOE off encodes
26 timer-clock dead-time ticks (406.25 ns). MOE is set only for drive or the
disabled-driver diagnostic. Dead-time is register-verified, not scope-measured.
Sine duties update at nominal 1 kHz, independently of the hardware carrier.

Capture retains 256 final drive records (256 ms): IA/IB/IC, VSENC, neutral, VBUS,
VREFINT and all three multiplexed comparator states. These are asynchronous
to PWM; current spikes are not PSU average current. `on_a/b/c` remain rounded
microsecond representations for replay compatibility, not exact CCR values.
Coast stores 500 records at approximately 2 kHz with gates/ENABLE disabled.
The compact coast records include measured time since disable; historical
`CTIME` dumps and older nominal-time dumps still replay. `TIMING` reports elapsed
drive time, control ticks, and the largest control interval.

The foreground wall-time cutoff is 4,999,000 us, allowing one control interval
of margin below five seconds; it is not an independent hardware watchdog.
The startup UART banner stretches the first control interval to about 7.64 ms.
Drive timestamps remain nominal control ticks; coast timestamps are measured
with TIM17. PWM edge timing is measured against that same MCU clock, not an
external calibrated instrument.

## Build / flash

Compile only: `cargo build --release --example shell-pwm`.
Flashing affects the physical rig; first send `off`, release the serial port,
then, with operator authorization:

```text
probe-rs download --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430 target/thumbv6m-none-eabi/release/examples/shell-pwm
probe-rs reset --chip STM32G071RBTx --probe 0483:374b:066CFF343433464757233430
```

Ascii85 release image measured 27,640 bytes text and 21,104 bytes static RAM
(stack additional) on the 36 KiB G071. Bench results: LAB_REPORT Entries 012–013.

## Snapshot wire version a85-v1

`adctiming` is an idle-only, driver-disabled diagnostic: measures 32 fixed
ADC2/ADC3 pairs at each SMPR setting using 64 MHz TIM1 count deltas, reports
min/max, and restores the normal long-sample setting. It never applies gate
drive. Entry 020 records timing; short-sample accuracy is not yet qualified.

`adcverify` keeps gates/ENABLE off, reports ADC/comparator configuration, and
compares separate reads with pairs for ADC2/3 and ADC6/13 at SMPR2 and7.
`adcsettle` also keeps drive disabled: conditions the ADC with long ADC2 or
ADC13, then records neutral's first/second/eighth short read and a long read,
averaged over32 trials. Both restore SMPR7. Entry023 establishes substantial
short-acquisition dependence on neutral; do not regard ON analog pairs as
an accurate voltage reference merely because they fit the pulse.
`adcsweep` extends that test to ADC2 and ADC3 at all eight SMPR settings;
output identifies ch/smp/prior and first/second/eighth/long means. `adcsettle`
now uses those same field names for its original ADC3/SMPR2 subset.
`adcsingle` keeps gates/MOE/ENABLE off and measures128 prepared single ADC3
conversions at SMPR4/5/6 against the running TIM1 timebase. It checks hypothetical
ON bounds26..383, not actual gate levels. Restores SMPR7; Entry032 records
setting5 fits but setting6 does not. This is timing evidence, not calibration.

Experimental spinning observation (not BEMF-qualified yet, Entry 018):

```text
python scripts/drv_capture.py --observe --tag my_observation
python scripts/drv_observation.py captures/my_observation.txt
python scripts/drv_crossings.py captures/my_observation.txt
python scripts/drv_phase_audit.py captures/my_observation.txt
```

This runs the known sine profile for 4.7 s, then at most 96 ms of fixed
six-step 50 eHz / 6%, then disables for coast capture.
Dense observer capacity is384 records with minimum sample interval100 us;
trace/guard execution makes the actual interval about200 us, filling the buffer
in about79 ms and ending before96 ms. Every record still includes current/bus/
VREF telemetry after its digital trace; these channels are not simultaneous.
Current release uses27,536 bytes static RAM, leaving9,328 bytes of36 KiB for
stack/other runtime use; stack high-water has not been measured (Entry028).
`obs1` arms the next run50/du60; `obs0` cancels. Any incoming byte aborts the short observation.
The fixture owns UART and does not transmit during it unless aborting.
The additional OBS/B85 frame is separately CRC-decoded by drv_observation;
drv_crossings writes per-visit state strings and transition brackets to
_crossings.csv. Rejected samples break adjacency; first/last visits are excluded
from summaries. Two valid samples on each side support a transition, but this
offline evidence test is not a rotor-lock classifier or minz control filter.
drv_phase_audit writes _phase_audit.json with expected/opposite directions,
crossing brackets and neighboring-cycle same-step period bounds. Expected raw
polarity follows minz edges_for(3,step-1), not rm32's generic rising flag.
the standard capture plot shows the preceding sine hold plus subsequent coast,
not the six-step interval. Use the `_obs.csv`/`_obs.png` artifacts for that.
OBS wire a85-v3 uses a prepared, SMPR=2 ADC2/ADC3 pair inside the ON pulse.
Wire a85-v4 additionally stores a second ON comparator read immediately
after the ADC pair in flags bit3. cnt_end now brackets that read too. v4 replay
reports early/late sign agreement only for floating-C, timing-valid samples
outside a 10-count analysis deadband (not calibrated analog uncertainty).
v5 is a digital-only observation experiment: no ADC2/3 acquisitions
during the six-step interval; flags bit4 marks absent analog values (65535).
Host reports C-N as unavailable, not zero. Early ON is unchanged; late ON is
read after waiting for count320; cnt_end brackets that read. All current/bus/
VREF acquisitions and guards remain. Sine/coast analog capture is unchanged.
v6 adds a PWM trace: legacy vc field is a 12-bit level mask and neutral
field a 12-bit miss mask, NOT analog readings. Bits correspond to timer counts
128,224,320,448,640,960,1600,2400,3200,4000,4800,5600. A comparator read is
valid only when bracketed within target..target+80 counts (1.25 us). Slots224
and448 are currently deliberately omitted/marked missed to reduce overrun.
cnt_end brackets slot320; global reject bit2 still vetoes legacy ON timing.
Slot320's allowed bracket extends past the PWM falling edge at384: do NOT
call this an ON-only measurement. Per-slot replay uses its own miss bit plus
commutation blanking, independent of the global legacy late-ON rejection.
Timestamp/sector age in v6 refer to the early read, before the trace sweep.
Current implementation stores the early raw TIM17 value and extends it after
the sweep; reconstruction has microsecond quantization/read skew and is not
a sub-microsecond timestamp. The two active-vector slots are explicit inlined
reads before bookkeeping, rather than generic-loop iterations (Entry027).
`obstiming` runs the same observer with ENABLE held low, then prints slot miss
counts and late-read bounds. It never enables the driver; MCU PWM pins toggle.
Two unused slots224/448 remain explicitly marked missed, not silently filled.
Full trace extends to about89 us; slow ADC guard telemetry follows it. This
increases foreground commutation/guard latency; no tighter timing is claimed.
Current v7 also records the shared observe-only minz-based detector: flags
bits5/6/7/8 are expected/armed/candidate/latched; reject bits4..6 carry reason
0invalid,1needs-baseline,2opposite,3accumulating,4candidate,5already-latched.
Existing timing reject bits0..3 and digital-mask meanings remain unchanged.
Detector execution is after the timed reads and never alters commutation.
tools/observer-replay verifies every v7 decision against the identical source
on the host. Candidates are not proof of tracking or permission to close loop.
cnt/cnt_end bracket conversion/read; us and sector_us are measured after the
pair. ON comparator is read just before conversion, OFF comparator after
count 1600, and currents/bus/VREF later with SMPR=7. These are not simultaneous.
Reject bits: 1 commutation age below 200 us; 2 pair outside counts 26..383;
4 slow telemetry end count below start count (not a reliable full-wrap detector);
8 pair end count below start. Short-sample analog accuracy remains unqualified.
Historical v2 measured the pair in the OFF window, with an earlier cached ON
comparator; v1 cnt_end covered the whole telemetry scan. Replay retains versions.
None changes CAP/COAST framing. See Entries 018-021 for limitations.

`adcwindow` keeps ENABLE low and checks 128 prepared pairs at SMPR=2 and 3
against 6% source PWM, including source-input level before/after conversion.
It restores SMPR=7 and clears gates afterward. This qualifies MCU timing,
not energized analog settling. Entry 021 records preflight and spinning results.

`sixcheck` is a disabled-driver six-step preflight (ENABLE stays low). It
tests each rm32 sector for 1 ms at 6% PWM, reporting OR/AND of the six MCU
gate-input levels against expected masks. It restores sine TIM1 configuration
and clears MOE/CCRs afterward. It does not spin or qualify energized BEMF.

Current `shell-pwm` captures use logical IA/IB/IC = ADC4/ADC1/ADC0, based on
Entry 016's directed-pair tests. `IMAP ia=4 ib=1 ic=0` records this mapping
before the CAP header. Older captures without IMAP retain their historical
labels (IA=ADC0 and IC=ADC4); do not compare phase identities without accounting
for that difference. `sensezero`, `map` and `pair` raw diagnostics retain ADC
numbering independently of the logical capture map.

`pair0`..`pair5` are short connected-motor identity tests for A>B, B>A, A>C,
C>A, B>C, C>B. They use 6% / 10 kHz, at most 16 OFF-window scans and a 1.9 ms
foreground cutoff, then disable and report zero-subtracted ADC statistics.
Abort checks: nFAULT, >800 raw-count current deviation, or bus below 70% of
the pre-burst reading; bus0 below 800 counts refuses drive. These are diagnostic
limits, not a validated average-current controller. No bulk encoding while
energized. Replay saved results with `scripts/drv_current_identity.py <file>`.

**Unloaded-only diagnostics:** `map0`..`map5` test AH/BH/CH/AL/BL/CL one
input at a time. Motor MUST be unplugged. These use about 810 us static
assertions, not the capped motor waveform. Each returns three before/during/
after records and restores MOE/ENABLE off. Do not use them with the motor
connected. Replay with `scripts/drv_phase_map.py --infile <capture.txt>`;
live fixture requires `--motor-disconnected --out <capture.txt>`.

Idle `sensezero` measures current-amplifier offsets without gate drive: ENABLE
wakes for 2 ms, up to 32 scans are buffered as min/mean/max, then ENABLE goes
low before printing. Channels are IA=0, IB=1, IC=4, VSENC=2, neutral=3,
VBUS=6, VREFINT=13. `wake_fault` and `scan_fault` distinguish the two periods.
This is not a gain calibration or a phase-identity test (Entry 014).

`WIRE a85-v1` precedes the existing CAP header. Each `D85 ` line contains
15 little-endian u16 fields in header order plus a little-endian CRC32/ISO-HDLC
over those 30 payload bytes, all encoded as standard Ascii85 (no Adobe wrapper).
Each `C85 ` line contains the six coast u16 fields, a little-endian u32 elapsed
microsecond timestamp, and its CRC32. CAP/COAST END delimit sections; the host
checks record lengths, CRCs, counts, ordering, timestamps and field schemas.
Headers are textual and are not CRC-protected. The firmware encodes one record
at a time without heap allocation, only after the bridge is disabled.

Host checks: `python -m unittest discover -s scripts -p test_drv_capture.py`.
Standalone encoder tests: compile `examples/support/snapshot.rs` with host
`rustc --edition 2024 --test`, then execute the resulting test binary.
# Flying acquisition probe (Entry 105)

`guardcheck` (Entry111) is an idle, bridge-disabled test of the independent
TIM6 guard and DRV safing path. It uses explicitly synthetic feedback to
exercise stale-feedback, missing-event and inherited-deadline trips, printing
three compact `POWERGUARD` summaries. It never enables the bridge and does
not qualify real ADC current limits or powered motor shutdown.

`flycheck` scans the comparator only with the bridge already disabled.
`coastfly1` arms the same scan after the next normal drive exit;
`coastfly0` cancels it. Do not combine with `coastref`: both probes are
explicitly refused if simultaneously armed. This probe reports a measured
rotor seed only, with **no commutation authority**.

Compact `FLY` results: 1 seed, 2 invalid phase, 3 wrong edge order,
4 interval too fast, 5 too slow, 6 expired, 7 sampling gap,
8 unsafe bridge/fault, 9 abnormal drive exit, 10 conflicting probes.
Times named `ticks` are half microseconds. Experimental mux settle is10us;
changed levels require repeated sampling with >=20us persistence and <=100us
per-phase gaps. A seed requires12 ordered intervals of1333..2000ticks.
This is acquisition qualification, not proof of sustained reference lock.

Bridge-disabled bench-tested in Entry107: `coasttrack1` arms flying
acquisition followed immediately by the actual reference controller with
sense-mux authority only; `coasttrack0` cancels. Gate authority remains zero.
`coastfly1/0` also clears the tracking request. FLY result11 means the measured
seed could not be handed off (invalid/late seed or disabled-state check).
Reference histories use the measured interval, and first COM timing subtracts
edge age. A >=32us remaining-arm margin is required. `CORESEED assumed=0`
reports measured interval, age and remaining ARR. Bootstrap COM is explicitly
not an EV_ACC; later COMs require real reference input acceptance.

### Sustained powered runs (Entry129, 2026-09-13)

`engagemsN` selects the powered-reference window in milliseconds,20..600000,
idle only; default1000. This is separate from the bounded open-loop startup.
`engageduN` selects independent handoff duty in tenths of a percent: startup
at6.5% and handoff at4.5% is currently the measured sustained operating point.
Do not lower startup to4.5%; these are different waveforms and operating states.
`engage1` arms the next run. Any UART input during powered execution aborts;
live speed/duty changes during this segment are NOT yet implemented.

Reproduce the10-second powered experiment (startup adds approximately4.7s):

```text
python scripts/drv_capture.py --target 50 --duty-tenths 65 --catch-duty-tenths 65 --engage-duty-tenths 45 --engage-ms 10000 --step-targets 60,70,80,90,100,110,120,130,140,150,160,170,180,190,200 --tag UNIQUE
```

The fixture checks the duration handshake, adjusts its timeout, preserves raw
evidence, clears arming/duty overrides, restores engagems1000, and verifies off.
There is no speed governor: this setting settles near236-238eHz, not200eHz.
Two full10-second runs succeeded; the third attempt failed during handoff.

Fixture-only A85 startup/T85 suffix records retain first/last32 accepted events.
`python scripts/drv_accepted_events.py CAPTURE.txt --windows` verifies CRC and
window accounting, exposing omitted-middle count. Default replay still rejects
incomplete streams. ACCEPTQUALITY and ACCEPTMOMENTS cover all accepted records;
host mean=sum/n and variance=squares/n-mean^2. Whole-run same-sector cycle
variation is more meaningful than assuming six equal inter-event gaps.
These moments include startup transients; they are not steady-state-only jitter.
ACCEPTTIMING is the historical narrow-band prefix-only report, not the powered
guard verdict; do not use its stale flag after prefix overflow as a dropout.
No qzc/rotor-lock certificate is inferred from accepted/COM ratio alone.

`--core-trace 0|1` explicitly selects per-read IRQ instrumentation; cleanup
disables it. Trace1 includes the first32 CRC-protected I85 handler records;
tracing changes persistence-loop timing and is not a free observer.
`flyseed0..6` / fixture `--seed-sector 0..6` is diagnostic only:0 accepts any
sector,1..6 waits for that first real edge then qualifies twelve intervals.
The20ms acquisition deadline is not restarted. FLYSELECT reports skipped
initial edges. Cleanup restores0. This selection adds coast time and must not
be used to conceal bad sectors. Entry131's selected-sector cohort did not prove
tracing causes the early handoff failures.

`capstride1..100` / fixture `--capture-stride N` changes retained startup records
only; ADC scans and current/bus checks remain1kHz. Default/cleanup1. Stride19
fits nearly the entire4.7s startup in existing256-record RAM. Every ADC-detected
fault sample is retained even between regular log ticks; asynchronous fault
paths need not have an ADC sample. CAPCADENCE records the exact divisor; legacy
integer sample_hz is rounded down, so use each record's original tick/time_ms
for analysis. Sparse records cannot resolve individual BEMF crossings or prove
rotation by themselves; use independent coast evidence. `comtiming` now needs
the existing `bench-adc-probes` feature; normal motor guards remain included.

Entry133 extends measured sustained operation down to~206eHz: unchanged6.5%
startup with4.0% powered handoff,trace0,any seed. Fixed8-attempt cohort had five
complete10s runs and three acquisition refusals; do not call entry8/8 reliable.
For reproducible all-attempt accounting:

```text
python scripts/drv_sustained_report.py --prefix sustain40_stepped_sparse_ --out NEW_REPORT.csv
```

The offline report verifies ADC-frame CRC, accepted-window accounting and off
readback AFTER COAST END. It distinguishes startup stop/acquisition refusal/
powered stop/window completion. COASTREF7 plus POWERPATH2 at the selected
deadline is normal timer-owned completion, not automatically a dropout.
Cycle mean/sigma includes startup and is not an independent qzc/lock certificate.
