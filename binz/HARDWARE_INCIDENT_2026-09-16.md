# Hardware incident - 2026-09-16

## Status

After E872, the operator switched off the bench and reported that something
burnt, imposing the E873 hard stop. The old motor was subsequently localized
as the only hot component and replaced. On 2026-09-18 the operator authorized
the bounded E877 recommissioning check; it passed at a10% cap. The original
failure mechanism remains unresolved, so the5-50% campaign is still paused
even though gentle operation of the replacement-motor stack is now proven.

The binz5-50% campaign is incomplete. E872 reached50% command for roughly23s
but did not qualify it.

Operator update E874: the motor was the only component that became hot and is
the apparent source of the burnt smell. Motor winding, lead or connector damage
is now the leading hypothesis. Temperature localization alone is not proof;
the motor was also the intended load.

Operator update E875 (2026-09-18): the motor was replaced and the operator
authorized a single gentle operational check capped at10%. Serial MCP recovered
and enumerated COM41, but the target returned zero bytes to two MCP-only
`off/p/i` attempts with both CRLF and CR line endings. COM41 was closed. No run,
gate, ENABLE, flash or motor command was issued. Recommissioning waits for
confirmation that the board/PSU is powered and for a valid idle readback; port
enumeration by itself is not a health check. Python serial is not an allowed
fallback when MCP is unavailable.

Operator update E876: a dedicated motor-inert USART3 uppercase-echo image was
built successfully but was not flashed. Its standalone ELF and explicit G071
probe selection were correct; the probe returned repeated `SwdApWait` and
failed during connection, before any erase or program operation. Together with
the silent COM41 target, this points to missing target power or an unreachable
MCU/debug connection. The previous E867 image is presumed unchanged but cannot
be freshly verified. No motor command was issued.

Operator update E877: after the operator power-cycled the boards, COM41 and the
existing E867 shell recovered, so the safe-UART image was not flashed. Idle
readback showed all gate commands, ENABLE, TIM1 MOE and CCRs off and nFAULT
high. With the replacement motor, one bounded recommissioning run used6.2%
forced startup, an exactly10.0% BEMF cap, and a5s powered window. An MCP-local
ACK-gated ramp reached200eHz before handoff. The handoff succeeded and the run
completed its deadline with16008 powered commits, no tracking/event fault, no
IRQ overrun, bus minimum11.617V, and no current, low-bus or nFAULT stop. A final
independent `off/p/i` readback again showed every gate/ENABLE/MOE/CCR off and
nFAULT high. This is a post-incident operational check only, not10% campaign
qualification and not attribution or clearance of the E873 burn mechanism.

## Last retained evidence

Capture: `captures/duty50_872_e867_level_revisit_ramp50_05s_60s_01.txt`.

- E867 diagnostic level-revisit image, SHA
  `A910E9DE9B8193C9201EE0C4861D6D86BCF526DBA96A4B0CA075872E15760F45`.
- Power stop: `reason=25` at powered time51.022318s.
- The bus guard accumulated50 coherent scans and tripped because the block's
  cross-product average was below its fixed ratio threshold. The110 low-bus
  codes are cumulative across the run; cause5 raw bus/vref/vcal863/1508/1662
  is the final trip sample, not the block average.
- Current diagnostic was report-only; max residual18812 against nominal
  allowance17308. These are uncalibrated internal counts, not amperes.
- No current foldback action, leading nFAULT, Tracking8 or accepted-event watch
  violation was reported.
- Final complete electrical cycles were646..665us, substantially slower than
  the515..565us suffix at45%.
- The post-run fixture commanded off and read gates, ENABLE, MOE and CCRs off;
  nFAULT was high. This predates the operator's burn report and is not a current
  health check.
- The capture's4.0A PSU label is stale build metadata. The operator reported
  the physical PSU setting as4.5A before E872.

This evidence establishes a low-bus protection event immediately before the
physical incident. It does not distinguish a connector/lead fault, motor or
winding heating, driver/FET damage, capacitor failure, supply fault, or another
power-path defect.

## Offline E873 decode

The existing decoder validated the compact-record CRCs and produced:

- `captures/duty50_873_offline_incident_decode.csv`
- `captures/duty50_873_offline_incident_decode_coast.csv`
- `captures/duty50_873_offline_incident_decode.png`

The246 drive-ring records span startup ticks19..4674ms. They end long before
the51.022s power stop, so their11.50..11.95V range is not evidence about the
terminal event.

The independent2kHz bridge-disabled coast recorder begins immediately after
shutdown and does cover the event's release:

| elapsed after disable | decoded VBUS |
|---:|---:|
|0.506ms|8.796V|
|1.010ms|9.628V|
|1.514ms|10.645V|
|2.018ms|11.371V|
|2.522ms|11.801V|
|3.025ms|12.023V|

It later settles around11.6..11.8V. This independently corroborates a real
loaded bus collapse that relaxed within a few milliseconds after gate disable.
That timing is compatible with several sources: PSU current limiting, a high-
resistance or heat-damaged connection, or a bridge/motor current event. It is
not component attribution. The coast comparator stream is not used for a rotor
state claim here because its sequential phase reads did not yield a qualified
debounced sequence.

## Preserve before inspection

1. Leave bench power off and isolate its output. Leave all auxiliary target
   cables disconnected so a damaged rail cannot be back-powered.
2. Allow the rig to cool fully and ventilate. If there is smoke, heat, swelling
   or an active fire, clear the area and use an extinguisher appropriate for
   energized electronics; do not touch or probe it.
3. Before moving wires, photograph the whole setup and close views of the PSU
   terminals, leads, connectors, DRV board, Nucleo wiring and motor connector.
4. Record where odor, discoloration, melted insulation, soot, bulging or a hot
   spot is strongest. Do not treat smell alone as component attribution.

## Re-energization gate

The operator performed the E877 replacement-motor recommissioning run. Before
any return to the5-50% campaign, the unresolved incident gate remains:

- the damaged or overheated element and affected path are identified;
- charred, melted or suspect parts and wiring are removed or replaced;
- with all sources disconnected and capacitors discharged, power-off checks
  show no unintended VM-to-GND short, no phase-to-chassis/ground short, and
  comparable motor phase-to-phase resistance;
- the driver/power board can be tested separately from the motor with a
  current-limited staged-power plan;
- the first powered test starts at a low current limit with no gate drive and
  verifies rails, idle current, ENABLE-off behavior and nFAULT before any motor
  command.

Do not reuse E872's clean firmware readback as evidence for any item above.

## Next power-off motor checks

Perform these only after the motor has cooled, with its3-wire plug disconnected
from the driver and every power/debug cable removed:

1. Photograph and inspect the motor plug, solder joints and lead insulation for
   darkening, melting or a loose/high-resistance contact.
2. Turn the rotor by hand. Record new scraping, binding, rough bearings or
   magnetic cogging that is markedly uneven.
3. Short the meter probes together and record lead resistance. Then measure
   all three phase pairs AB, BC and CA using the same pressure and range. Small
   absolute readings may be dominated by probe/contact resistance; compare the
   three readings and repeat them for stability.
4. Measure A-to-case, B-to-case and C-to-case. Each must remain open. Do not use
   a high-voltage insulation tester unless the motor manufacturer permits it.
5. If any pair is materially different, unstable, or any phase conducts to the
   case, quarantine the motor. Do not reconnect it to the DRV board.

Even equal DC phase resistance does not clear a turn-to-turn short; that can
hide below a handheld meter's resolution. Burnt winding odor, visible varnish
damage, unequal inductance/back-EMF, or abnormal no-load current would require
replacement or a separately protected motor test—not this ESC rig.
