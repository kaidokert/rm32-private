# AM32 build for NUCLEO-G071RB + BOOSTXL-DRV8304H

## E533 — bootstrap sector coherence correction does not fix entry

Source: global step1/rising1; UART/fixed boot previously called comStep(2)
without changing step or comparator mux. Board-only bootstrap now calls
comStep(step),changeCompInput while PD1 disabled. Existing controller startup
and timeout logic remain. This corrects an internal initialization mismatch;
not proof it caused failed starts (idle allOff subsequently clears drive too).

Read locally cached drv8304.pdf pp6/35: wake specified1ms; existing delayMicros1000
uses utility timer. No measured wake violation established; did not lengthen
delay or claim hardware fault. Source polling startup has old_routine1 and
normal22.5ms timeout re-kick; initial gate write alone is not a driven-start seed.

Built nativeO3 FLASH24544/RAM3960, emitted math audit; installed
02592720FB8A1DCA2BEA0F2915BED553FE3399ECA2DBE8E541D2D5804D4AB2FC.
One10input2s attempt FAILED avg16126,zc0,running0,CCR0; finalPD1ODR0,
empty segment header valid. captures/am32_533_bootsector_input10_2s_01.txt
retained, no identical retry. Three host decoder tests pass. No speed/current
guard changes. Do not describe this as startup fixed; no further speculative
startup tweaks without first-drive evidence. Board this image/off, source matches.

## E532 — lower input cohort fails; frozen build also fails startup

Predeclared fixed order10,7,7,10,10,7, each2s with stop between, on E53197307B38.
All six had finalzc0/running0/CCR0 and verifiedPD1low; 0/3 success each input.
This does not prove no transient movement; it rejects lower input as an observed
startup fix in this cohort. No firmware changes or threshold adjustments.

| Attempt | Input | Final average ticks (not running speed) | Capture SHA256 |
|---|---:|---:|---|
|1|10|20434|2D3F7517E41B87E9D1EDCE76986FC0D64635A793BC6C26CCDD8DFCAFBE85D355|
|2|7|20920|5762E5245704E6222B5DCE3F75CF9EB4A68630882990B31CB7D4C6C379BC6405|
|3|7|20866|EEF7B7A20B69C6506DBD45CDCE91FF4B045CD734C8EC5DE1B6630A00F6DA07A4|
|4|10|19766|61AA156373264E997524D33F127AC97F1DA75B5636711106EB9D5F3324381A5B|
|5|10|16419|7171B346B058E098F87AFDB5804ABE0C93FAA7CDF4D1A88DDB476C0391A02D70|
|6|7|20899|0F3ECF394D5AD89A06A56C8463BABEC2678AFBC5B231F196F2D659A041FF3B09|

Captures am32_532_start_<attempt>_input<input>_2s.txt. Same boot cohort;
not independent reset trials. Hardware inhibit confirmed after every attempt.
Restored exact frozen E530E2BFFF04 via download/OpenOCD reset, then one10input2s
also FAILED avg16497,zc0/running0/CCR0/PD1low. Capture
am32_532_frozen530_input10_2s_01.txt SHA256
E3BFADA1B0103AFF6B7300BF8135057091DAB022AD304B878BA7D82E45B6BA0D.
Snapshot addition is not necessary for failure; not proof it has zero effect.

Stopped SWD on restored E530: UART200004c0=0,adjusted200004c2=0,
latch200004c4=0,PD1ODR50000c14=0. ADC_raw_volts2000050c=0x04cb;
ADCDataDMA20000764 first3 halfwords043c/04cb/03ae. GPIOB_IDR00000050,
captured with driver disabled; do not infer enabled nFAULT state from this.
ADC channel4/6/temp mapping source checked, but no loaded-bus/current calibration
claim. No SWD during powered runs. Earlier successful741eHz observation remains,
but startup reliability not established on either build. Next inspect initial
commutation/startup state and timing, not another speed-ramp or hardware blame.
Actual board frozenE530/off; obj ELF/source remain E53197307B38, NOT same build.

## E531 — command-boundary snapshots localize this failure to startup

Added foreground-only RAM8x7 words at nonzero throttle transitions before
updating UART input. Fields: prior encoded input,average_interval,zero_crosses,
ARR,CCR1,running,bemf_timeout_happened. No IRQ masking, no additional sensing,
no TX while enabled. Sequential reads are NOT atomic and do not measure jitter.
After existing AM32STOP, AM32SEGS gives count/overflow then AM32SEG rows.
Final segment is the stop snapshot. Host --segments requires exact input order,
count, bounds, no overflow; later patch also refuses any failed earlier endpoint.
No CRC/continuous lock claim. Added3 decoder tests including retained failure.

NativeO3 FLASH24536/RAM3960, math-audited installed ELF
97307B38AAD24A9E0213661E4A673307286D3763D3B5429791C9B95363BDCAB7.
Disabled stop/header validated. One diagnostic10/15/20/25/30 ramp1.5s perstep
FAILED: all four command-boundary rows report avg16101,zc0,CCR0,running0,
stuck-rotor latch102. First row belongs to10input (encoded247). Final stop
also zc0/running0/CCR0/PD1ODR0. Thus failure exists BEFORE leaving10segment,
not high-speed loss at30. Cannot retrospectively locate E530 failure or prove
there was never any brief initial movement. No identical retry.
Capture captures/am32_531_segments_ramp10to30_01.txt SHA256
B48F37E33D4E46528990887C811C4E221C7D1856E5706B633F6E05D9DD30E29E.
Board disabled, UARTclosed. Next examine startup, not speed-ceiling tuning.
Binz CycleTiming source audit confirms same-sector accepted-event period limit,
not CPU occupancy detector; no binz firmware or guard changes this entry.

## E530 — decoded UART bypass; known-settings response beyond binz envelope

Source evidence: bench boot sets newinput48. With bidirection1, DShot decoding
interprets48 as reverse and can reset zero_crosses/old_routine/maskCOMP while
changing forward. E529 only restored adjusted_input afterward, not side effects.
Board-only setInput now bypasses receiver decoding altogether, supplies decoded
UART throttle, and leaves configured boot direction unchanged. Stock protection
and comparator/commutation logic unchanged. Config remains known defaults plus
bidirection1, sine/brake/car disabled. This is a UART adapter correction, not
proof that reverse operation caused every earlier startup failure.

Native O3 build FLASH24032/RAM3728; emitted math audit generated. Installed ELF
E2BFFF04D90DB475173018357E4DC271B5F80FC0868022BF3A26774713E61E99, frozen with
disassembly/audit under captures/reference/am32_e530_e2bf. No probe during runs.
800mA PSU setting retained, no new measured supply/current values. Four attempts:

| Capture suffix after am32_530_uartdecode_ | Input sequence | Final average | eHz estimate | zc | CCR/2666 | Result |
|---|---|---:|---:|---:|---:|---|
|input10_2s_01.txt|10 for2s|1006|331.35|3991|312|response|
|ramp10to20_01.txt|10,15,20|569|585.82|10000|572|response|
|ramp10to25_01.txt|10,15,20,25|450|740.74|10000|707|response|
|ramp10to30_01.txt|10,15,20,25,30|17937|not valid running speed|0|0|FAILED|

Ramps1.5s per segment, no stop between steps. Final running1 for first three,
running0 for last. All final PD1low, no early stop acknowledgement; current
snapshot does not resolve failure stage. Do NOT label it failure at30 or assume
it started successfully. No identical retry. zc saturates10000 and is stock
progress, not independent qZC. Actual PWM first three11.703/21.455/26.519%;
hard output cap remains30%. These are short response tests, not sustained lock.

SHA256 captures in table order:
3BE4BC268326FAE764DD4361F58CDF2D92D029FB8B1ABA03BE73095359E30089;
8F72144B7C20D407E6CAE8CECDF55F5DAB3EC0EBAC13EBDB8B2185533386DC9A;
A1C1ED93641BC45D2FCA6964D9FBD800EA6D1CF6766D92ECCA3DE1D13B2A5B30;
3E40BBD2DA301000457BCBFFF622BE4C696A7F3D92EDBA964FC841A00C292C90.
Stopped SWD confirms forward20000052=1,uart_input200004c0=0,
adjusted_input200004c2=0,bemf_timeout_happened200004c4=0,PD1ODR0.
Useful control evidence now exists on known settings above334eHz. Next preserve
reference and identify failed ramp stage with bounded post-run-only segment
snapshots if further AM32 work is needed; do not spend another campaign on
instrument micro-optimization or infer hardware limits from this failure.

## E528-529 — startup configuration and UART stop correction

E528 changes only known-default bi_direction to1. This changes startup filter
and changeover/direction mapping together; not an isolated filter mechanism.
Image19D90C45B8C453B3DF18921D4332AF748D8CEA095168F5735C9AA25208340E78.
Retained attempts, each stopped with PD1low:

| Capture in captures/ | Attempt | Final result |
|---|---|---|
|am32_528_bidir_input10_2s_01.txt|10 input,2s|avg998,334eHz,zc3931,running1,CCR312/2666|
|am32_528_bidir_input20_2s_01.txt|20 input,2s|avg15316,zc4: entry failed|
|am32_528_bidir_ramp10to20_01.txt|10/15/20,1.5s each|avg15734,zc0,running0: failed|

No live segment telemetry: ramp does not prove successful10% then loss later.
Stopped SWD after failures found UART input0 but adjusted_input46 and stock
bemf_timeout_happened102. Receiver mapping overwrites the UART stop value;
the main-loop zero-input reset therefore cannot reliably clear the latch.
E529 mirrors uart_duty_get after receiver mapping, before stock stuck-rotor
guard. No guard threshold or comparator/commutation algorithm change.

Built nativeO3, math-audited and installed ELF
BC0772AD4E83A485374D077FE42297734B37EF0B4CA37C5EF2699CB9D2454B8A.
One10input2s attempt still FAILED: avg15743,zc0,running0,CCR0,PD1low.
Capture am32_529_uartmirror_input10_2s_01.txt SHA256
5513277288D2670A596F6AEAC6893ECFE61788CB9063A56C0B4C8840AD8340F5.
Post-stop read-only SWD confirms adjusted_input200004a4=0,
bemf_timeout_happened200004b2=0,uart_duty_input200004b0=0,PD1ODR=0.
Stop/latch reset corrected; reliable startup NOT established. No further
25/30 run on known configuration. Earlier E526 fast responses cannot be
called clean stock/default parity. Next investigate configuration/startup,
not another identical retry. Board remains this AM32 image, disabled.

## E527 — contaminated configuration found; known-config entry currently fails

Read-only stopped SWD audit of7826 image showed temp_advance0x16 (22),
and eepromBuffer at20000550 began:
`eb 03 c8 02 14 48 68 44 ff aa 2c 32 c3 49 01 f0 29 f9 05 00 d5 ff 03 20 01 24 00 26 00 90 21 46 32 46 05 f0 bf fc 00 00 05 0a 0a ff 01 05 00 f0`.
BDTR0200a03c,PD1ODR0. Prior binz84DF ELF .text at0800f800:
`eb a9 c8 67 c4 48 68 44 ff aa 2c 32 c3 49 01 f0 29 f9 05 f0 d5 ff 03 20 01 24 00 26 bd 90 21 46 32 46 05 f0 bf fc fa f7 9b fe 00 28 01 d1 00 f0`.
Matching instruction bytes plus expected version/loader/bench field edits
identify foreign code consumed as settings. loadEEpromSettings unconditionally
reads192 flash bytes and validates only selected fields; changing version tags
does not initialize the rest. E524-526 were not default-settings AM32 runs.
Notably auto_advanceF0 and bi_direction05 are true; comp_pwmD5 is also true,
motor_poles38 exceeds32, and limits.current1 enables the current limiter.
Thus the assumed current-limit-OFF state of those runs was not established.
Their observed response beyond334 remains evidence, but not clean parity.

Actual AM32 emitted persistence loop calls getCompOutputLevel once/read, with
filter/rising loads inside iterations; binz's static loop does neither. Source
filter mapping itself still matches, so do not equate loop count with duration.

Board-only loader now bypasses flash: uses exact48-byte default_settings from
Src/DroneCAN/DroneCAN.c (documented2.19 config), zeros remaining192-byte buffer,
sets current version fields, then invokes normal settings interpretation.
No EEPROM erase needed. Six-step/brake/car board overrides remain. Fixed
advance16/auto0,compPWM1; current102 means default software limiter OFF.
Built/installed current986157652613BE207FCAB270E8302A015376FE055FB7FA359B62101A5B144532,
nativeO3,24832B FLASH,3736B RAM; emitted math audit69calls, advisory only.

Two bounded2s attempts at input10 and20 both fail useful entry: zero_crosses2,
running1, avg16172/15378 (~20.61/21.68eHz), PWM compare312/534 over2666.
No early stop, finalPD1ODR0, UARTclosed. The original fixture's zc>0 criterion
printed exit0; this is explicitly NOT response success. Tightened it to require
progress past20 startup counts (still not sustained/independent lock proof).
captures/am32_527_defaults_input10_2s_01.txt SHA256
`E75A2B507E0EDEE003E92D35192B003074F5EF1466542C01D81AEF16EFDC3AC6`;
input20 SHA256 `549C4D9FBFAC5807C25685520AEFEF3808694AE80F331254EDCA615B97C9C11C`.
No30attempt, no further duty expansion. Configuration changed many fields;
do not pick one cause without bisection. Next compare startup/mode behavior
between known configuration and the previously working configuration, while
keeping malformed flash data out of the loader. AM32 remains installed/off.

## E524-526 actual bench response — 2026-09-14 (latest)

Operator explicitly authorized PSU-only AM32 tests up to20% input, then25/30%.
800mA PSU setting retained; no new PSU measurement available. This is a
reference-test exception, not removal of binz's goal safeguards.

Board-only adaptations in the AM32 tree: standalone link via
`--defsym=drv_bench_standalone=1`, target initAfterJump VTOR08000000;
PD1 boots low and arms on valid nonzero UART command; s/deadman drop PD1;
HardFault drops PD1 and MOE. USART3 PC10 TX adds post-stop-only hex snapshot.
Command rejects beyond input limit; compare clamp covers combined PWM writes
and individual setters. RAM settings disable sine start/braking/car reverse.
Comparator/commutation routines were not edited. Native AM32 O3 build retained,
not binz opt-s/LTO; emitted math audit current ELF has71 helper calls (advisory).

Build ELF directly (generic BIN packaging failed because python3 is absent):
`make obj/AM32_DRV8304H_G071_2.20.elf "LDFLAGS_COMMON=-specs=nano.specs -lnosys -Wl,--gc-sections -Wl,--print-memory-usage -Wl,--defsym=drv_bench_standalone=1"`
Use probe-rs download, then OpenOCD reset, serialized. Standalone vector section
and PT_LOAD start08000000. This target must now use the standalone link symbol.

E524 imageE195D23411EF8433E367BEC6255C9B9E20A8F96F79D99AF419332AA935543D53:
boot/stop ack passed.7%/10% two-second attempts exposed premature deadman.
The existing UART port incremented deadman in uart_duty_get, called by setInput
also from unrestricted foreground.7% capture contains an early AM32STOP;
its initial fixture success is INVALID as full-window evidence.10% correctly
refused no-running/no-progress. Both stopped PD1low, retained captures.

E525 fixes deadman advancement into periodic tenKhzRoutine/TIM6 only, while
keeping command consumption/control algorithm. TIM6 PSC63 ARR50 gives~19.608kHz,
so60000 ticks is~3.06s, not exactly3s. Fixture sends no refresh in2s runs.
Image039C3A56E17057DAFBAC4EA8A1D9B9C9191495135D825983927FFAAB2EC7EDAE:

| Input | Interval ticks (0.5us) | Estimated eHz | PWM compare % | zero_crosses |
|---|---:|---:|---:|---:|
|10%|1244|267.95|9.9025|3242|
|15%|772|431.78|14.9287|1895|
|20%|591|564.02|19.9175|6646|

Each2s test reported running1, no early stop line, final PD1ODR0.
E526 changes only command/PWM ceilings20->30 and fixture accepted range:
current installed ELF SHA256
`7826B72DD521DC7E1CB4F2DE59C84A0BBAC6009A3D0D89DC2AD8CEBC526F1DD7`.
FLASH24944B plus vector/name, RAM3736B. Stop verified before flash.

| Input | Interval ticks | Estimated eHz | CCR1 / (ARR+1) | zero_crosses |
|---|---:|---:|---:|---:|
|25%|485|687.29|667/2666 =25.0188%|8151|
|30%|401|831.26|799/2666 =29.9700%|9615|

Again2s each, running1 at stop, no premature stop report, PD1ODR0 after stop.
UART closed, AM32 remains installed. Raw captures:
`captures/am32_526_input25_2s_01.txt` SHA256
`D34EA905EAEB4ACE84971FD8C130ED3335B3DA1673AE473A5BBCA00D828A2A6B`;
`captures/am32_526_input30_2s_01.txt` SHA256
`68DC148DC7CE23D6E0DEFA71C0666F68E069EEF7B492C5E031960543FD327520`.

Meaning: same-board AM32-derived response now exceeds binz's334eHz qualified
point. These short runs and one stop snapshot do not establish sustained lock,
independent physical RPM/qZC, current/power parity, or whether AM32 exhibits the
same late/short pair. No claim of perfectly stock firmware: adaptations above
matter. AM32STOP is unchecksummed text, not the binz CRC event archive. Actual
PD1 register-low is driver-disable readback, not multimeter proof of every gate.
No further duty increase authorized beyond30%.

## E523 image/boot audit — 2026-09-14

Read-only, no bench connection. Prepared ELF SHA256:
`0C83D8DAA23726063731A143618E9619139F8454F3A1134057BEAE0F05DE2122`.
AM32 HEAD `b4d28af8553cd1f798d2ceb31fcb4ddd68e89734`, with existing dirty
targets.h/peripherals.c/main.c; HEAD alone is not the build's source identity.
ELF vector section is08001000, initial SP20004000, reset entry080042fd.
SystemInit sets VTOR08000000, but main calls initAfterJump before peripheral
initialization; that function sets VTOR08001000 then enables IRQs. Therefore
SystemInit's zero offset alone is NOT evidence of an interrupt-vector bug.
Standalone relocation would have to update both linker and initAfterJump.

First ELF PT_LOAD starts08000000/file offset0, including ELF header/padding
before the actual vector section at file offset1000. Do not assume a loader
will preserve the bootloader merely because .isr_vector starts08001000: verify
its handling of sections versus segments or use an explicitly ranged image.
No claim that the current flash tool necessarily writes that header; loader
behavior was not tested. No bootloader installation or app execution performed.

Operator choice from E522 remains unanswered. No permission to remove firmware
guards is inferred from automatic goal continuation. Board remains last verified
off with binz84DF (not powered-qualified); neither AM32 nor binz was flashed here.

## E522 readiness audit — 2026-09-14 (supersedes run suggestions below)

Existing build is not yet compatible with the active guarded campaign contract.
Read-only source audit; no AM32 flash or source modifications performed.

- UART values are input-domain commands, not an actual PWM-duty ceiling.
  main.c maps input to minimum_duty_cycle..2000 and applies startup/stall
  adjustments. A `30` command does not prove PWM <=30%; enforce the ceiling
  at output generation, including startup/sine/braking paths, for a guarded test.
- The parser treats0..100 as percent,101..1000 as permille. Thus `69` means
  69% input, NOT6.9%; fractional settings below10.1% cannot be represented.
  `250` means25% input (250permille), not25permille. None is calibrated RPM.
- USART3 initialization enables RX only. No existing readback proves effective
  PWM, accepted/COM progress, fault cause or final outputs-off on this port.
- `s`/deadman set input/adjusted_input to zero. They do not directly drop PD1.
  phaseouts.c allOff floats phases; peripheral init asserts PD1. Dedicated
  stop/fault behavior and readback need verification, not inference from input0.
- Current limit defaults off, current topology is single phase rather than
  the binz three-phase raw guard. VBUS conversion/LVC is not bench qualified.
  PSU800mA alone is not the firmware protection required by the active goal.
- HardFault handler loops without board-specific safing. Startup code contains
  motor-tone paths; do not assume flashing/reset is an electrically idle test.
- App vectors start08001000. Bootloader or standalone vector/startup handling
  must be established before executing it.

The next decision is a board-specific guarded AM32 comparison versus an explicitly
authorized PSU-only reference run. Do not silently choose the latter. Preserve
AM32's comparator/commutation algorithm when adding an outer safety envelope;
label those adaptations and measure them. No100% authorization: max30% actual
PWM remains the operator's limit. A successful spin alone would not establish
the late-event mechanism or independent qZC parity.

Stock AM32, minimally adapted to the free-wired DRV8304H bench stack. **Build-only —
this has never been flashed or run on the board.** Source changes live in the AM32 tree
(`E:/m/robot/esc/AM32`), not here.

## Build

```
cd E:/m/robot/esc/AM32
make DRV8304H_G071
# -> obj/AM32_DRV8304H_G071_2.20.{elf,hex,bin}
```
Result at time of writing: FLASH 24320 B / 63264 B (38.4%), RAM 3696 B (22.6%). Clean
build; stock `GEN_64K_G071` still builds (changes are all `#ifdef`-isolated, no regression).

## Why GEN_64K_G071 is the base
AM32's `GEN_64K_G071` target uses `HARDWARE_GROUP_G0_A`, which is the exact pin map rm32's
G071 code cloned and the map this board was wired to. So gates, throttle input and BEMF
comparator pins line up almost 1:1.

## Pin mapping (target `DRV8304H_G071`)
| Function | Pin(s) | vs stock G0_A |
|---|---|---|
| Gate A high/low | PA10 / PB1 | same |
| Gate B high/low | PA9 / PB0 | same |
| Gate C high/low | PA8 / PA7 | same |
| Throttle input | **UART: USART3_RX / PC11 (AF0) @115200** | `UART_DUTY_MODE` ported from the L431 bench (replaces the PB4 DShot/servo path) |
| VBUS sense | PA6 (ADC_IN6) | same |
| BEMF comp A / B / C | **PB3 / PB7 / PA2** | **A/B swapped** vs stock (stock is PB7/PB3/PA2) |
| DRV ENABLE (nSLEEP) | **PD1**, driven high at init | added (stock G0_A has none) |
| Current sense | **PA4 (ADC_IN4)** = phase-A CSA | stock default is PA5; see caveats |

The A/B comparator swap matches this board's measured VSEN landing (LAB_REPORT Entry 015:
gate A/PA10 → VSENA → PB3, gate B/PA9 → VSENB → PB7, gate C/PA8 → VSENC → PA2).

## Source changes (AM32 tree)
1. `Inc/targets.h`
   - New target block `#ifdef DRV8304H_G071` (FILE_NAME `DRV8304H_G071`, FIRMWARE_NAME
     `DRV8304 G071`, DEAD_TIME 60, `HARDWARE_GROUP_G0_DRV8304H`, `DRV8304H_BOARD` marker,
     current on IN4, MILLIVOLT_PER_AMP 70, CURRENT_OFFSET 1648, serial telem, 64K).
   - New group block `#ifdef HARDWARE_GROUP_G0_DRV8304H` = a copy of `G0_A` with the A/B
     comparator inputs swapped and `DRV_ENABLE_PORT/PIN = GPIOD / PIN_1`.
2. `Mcu/g071/Src/peripherals.c` — in `MX_GPIO_Init`, an `#ifdef DRV_ENABLE_PIN` block that
   clocks GPIOD, sets PD1 push-pull output, and drives it high (DRV leaves sleep). Confirmed
   present in the `.elf` (GPIOD base 0x50000C00 referenced).

Everything is gated on the new target's defines; no other AM32 target is affected.

## Throttle: UART commanding (no flight controller needed)
This fork has `UART_DUTY_MODE` (an ASCII throttle over UART, "minz-rig parity"), but stock
it is `#ifdef MCU_L431` and hardwired to USART2/PA2 @2 Mbaud/80 MHz — which on this board
would fight the BEMF-C comparator (PA2) and run the wrong baud. It is **ported here to G071**:
- Enabled for this target only (`#ifdef DRV8304H_BOARD → UART_DUTY_MODE`). L4-only
  `SPIKE_STATS`/`ZC_TRACE` stay off.
- RX on **USART3 / PC11 (AF0) @115200 8N1** — the bench's COM41 console pin (FTDI TX → PC11).
- Protocol: ASCII digits then CR/LF commit — `0..100` = %, `101..1000` = ‰; `s` = immediate
  stop; ~3 s deadman → stop. Same command path minz/binz use (parity in the control path).
- Landmines handled (from binz LAB_REPORT): `USART3EN` set explicitly with `|=` (clock does
  not survive reset on this board — E100/E223/E236); **115200 not 2 Mbaud** (G0 RDR is 1 byte,
  FIFO off, so a 2 Mbaud burst overruns the 20 kHz poll); PC11/AF0/clock cribbed from binz's
  qualified USART3 driver.

Source: `Src/main.c` — `UART_DUTY_MODE` enable block, `UD_UART/UD_RXNE/UD_ICR_NCF` macros, and
the `#elif defined(MCU_G071)` branch in `uart_duty_init()`; poll made register-agnostic.
L431 and stock-G071 builds both verified unchanged.

## Stubbed / adapted "problematic" parts
- **Current sense topology.** This board has three low-side CSA outputs (bidirectional,
  ~1.65 V idle, ~70 mV/A), not the DC-link shunt AM32 expects. The single AM32 current
  channel is aimed at the phase-A CSA (PA4) with a mid-rail offset; it is only meaningful
  with **current limiting OFF**, which is AM32's default (`use_current_limit = 0`). Do not
  enable a current limit without re-deriving the offset/scale.
- **VBUS calibration.** VBUS pin matches (PA6) but the divider ratio differs from a real
  ESC; `LOW_VOLTAGE_CUTOFF` is off by default, so this only affects telemetry/LVC if turned
  on. Set the divider before relying on LVC.
- **ENABLE.** Handled by driving PD1 (no rewire), rather than AM32's `PWM_ENABLE_BRIDGE`
  per-phase scheme (which this driver doesn't use).

## Before ever running it (NOT done here)
- **Bootloader/flash:** AM32 app links at `0x08001000` and expects the AM32 g071 bootloader
  at `0x08000000`. Either flash bootloader + app, or relocate to run standalone via SWD.
- **Verify the A/B comparator swap** against the live board before trusting commutation
  (it's inferred from LAB_REPORT E015, not re-measured under AM32).
- **MODE strap** must be 6× (MODE→GND) — already done in hardware on this board.
- Keep current limit and LVC off until their sense paths are calibrated for this board.
- **Throttle it over the console:** on COM41 (USART3/PC11, 115200), send e.g. `6\n` for 6%,
  `250\n` for 25% input, `s\n` to request stop; no command for ~3 s requests stop. No flight controller
  needed. Prop stationary at idle — the start gate creeps at minimum duty once armed.
- **PSU current limit is the ONLY overcurrent protection on the first run.** With current
  limiting off (necessary — the CSA idles mid-rail) there is no firmware overcurrent guard,
  and in 6× mode the DRV8304 inserts no dead-time — TIM1's `DEAD_TIME 60` is the sole
  shoot-through guard. Run current-limited, hand on the supply, ramp gently.

## Purpose
An A/B reference: run stock-ish AM32 on the same stack to see whether the DRV8304 +
DNP-filter-cap BEMF path is usable (AM32 spins closed-loop → sensor path is fine, the rm32
BEMF grind is tuning) or marginal (AM32 also struggles → hardware sense limit).
