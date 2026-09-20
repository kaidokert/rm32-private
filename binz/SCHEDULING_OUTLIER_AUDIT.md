# Scheduling outlier investigation - E390, 2026-09-14

## E391 reachable masked work and a concrete recorder cost

Reviewed powered foreground, feedback publisher, accepted recorder, guard/DMA
handlers and CPU wrappers. Relevant recurring mask regions:

| Region | Work while masked | Important distinction |
| --- | --- | --- |
| adc_stream::take | ownership check and one FIFO frame pop | conversion happens AFTER mask exits |
| powered_timer::feedback_inner | sampled clock and fresh guard feedback; first-delivery snapshot once | convert(raw,vcal) runs in foreground outside this mask |
| powered_timer::accepted | sampled clock, order/cycle checks, fault snapshot | no serial formatting |
| Obs::record EV_ACC | timing moments, six-bin timeline, prefix/tail publication | real diagnostic workload inside global mask |
| powered_timer::commit | guard poll plus role/GPIO/mux writes | safety transaction must remain indivisible |
| TIM6 guard and cpu_meter boundaries | guard/clock checks and union accounting | period is not execution duration; wrapper cost also matters |

Obs::record on archivedCC71 calls __aeabi_uidiv at08003a6e for
Timeline::push's(us/bin_us). This occurs every accepted event while masked,
not merely during serial dump. E383 RECORDGUARD max_us39 is a WALL bracket,
not an exact masked-duration or a measurement at the particular bad boundary.
Moment accumulation uses64bit sums but bounds squaring to32bits; do not call
every64bit field a software64bit multiplication.

Replaced six-bin index division with a bounded decision tree comparing
bin_us,2b,3b,4b,5b. Constructor limits b<=100M so every threshold fitsu32.
Preserves floor/clamp, all timestamps/orderrefusals/overruns and64byte layout.
Tests cover everytimestamp across three smallwindowlayouts, each threshold
andneighbors through600s, andu32::MAX overrun.213RustPASS. Compiled recorder
contains boundarycompares/multiplyby3/5 (single32bitMUL), no uidiv call;
remaining calls are moments, safety and panicpaths. This removes one concrete
masked cost, NOT proof of the outlier's cause or measured jitter improvement.

Candidate05BA9841616BE6A0EBE73869B0F99D00087101D0DEEF753B7A3BE7DC32207079
NOTFLASHED, includes E390ADCbin math. Release+autoauditPASS,text121996/data1104/
bss29324. ActualCC71,lastoffE389 unchanged. Next strictprovenance and disabled
checks, matched preceding6.8% recovery to measure recorder/COM/IRQ wall impact
before faultreplay or newtracing. Avoid conflating two arithmetic changes.

Existing I85 tail records COMP entry-ish time, sector/PWM, gate/readcounts,
levels, acceptance delta and handlerwall. It does NOT log guard/DMA/COM
entry/exit chronology or hardware edge time. Thus it cannot by itself name
which context preempted a persistence window. Trace1also changesadapter path.
Do not promise causal scheduling attribution from merely toggling it on.

### Addendum checked against graybeard's later NVIC memo

The actual qualified build uses bench-com-peer: COMP64/COM64, guard0/DMA0.
The memo's abbreviated table showing COM128 is not this installed profile.
Peer mode followed measured COM latency trouble; changing it is an A/B with
guard/starvation consequences, not merely restoring an assumed good priority.

Existing global IRQ masks still block priority0COMP too. The recorder divide
above is one example; priority changes alone cannot remove that exposure.
Repeated COMP delivery could defer a lower-priority guard longer than ONE
observed65us handler. Need starvation/backlog analysis and disabled fault tests,
not just65+100us arithmetic. Sum of observed maxima is neither WCET nor an
exact stacked-delay bound; nestedwall measurements can doublecount, and
unobserved paths/entryexit costs remain. Preserve all safety deadlines.

Carrier-period peaks support PWM-related qualification timing but do not
uniquely prove DMA/guard preemption. TIM6CNT at acceptedCOMPentry can reveal
correlation, not identify the original hardware edge or the interrupt that
preceded a delayed entry. Avoid claiming uniformhistogram exonerates either.

Host live_capture has no keepalive. It sends paced startup ehz targets on
nominal host3.2..4.4s schedule, then reads until COAST END; cleanup/timeout sends
off. Suppression of unsent startup targets uses the TIMING energized_us marker,
not a target-MCU ownership handshake. A severely stalled host could therefore
send a late startup update; successful targetRX would request HostAbort, not
explain a normal persistent run's CycleTiming by byte-length blocking. This
is a host-scheduling caveat, not proof it happened in E383. No hostchange here.

Operator explicitly rejects unsupported hardware-blame/cap proposals. No
hardware fault is proven; no wiring/cap change planned. Scheduling/acceptance
timing is a live hypothesis, not proven merely by a tight sigma and outlier.
Likewise, no stock-AM32 result exists on this rig yet; do not claim100% passes
or treat the operator's comparison suggestion as a new100% duty authorization.
Current campaign hard maximum remains30%, with all existing guards.

## UART path checked in current source

- shell-pwm USART3BasicConfig115200; powered callback passed through
  driven_run::run to driven_power_run is `serial.read().is_ok()`.
- HAL serial/usart.rs Rx::read reads ISR, checks error/RXNE, returns byte or
  WouldBlock. No byte-length wait or interrupt mask in this poll.
- core_bench::coast_run_inner's powered loop polls abort, services feedback,
  handles bounded injection/state/deadline checks and observe_bands. It does
  not receive a serial writer or format/transmit data. Dumps occur after stop.
- HAL fmt::Write does busy-wait on TXE via nb::block, but those functions do
  not mask interrupts. Sine startup banner flush precedes wave_timer::start.
  These are source facts, not a complete generated whole-program blackout proof.
- At115200 a byte occupies~87us on the wire; that does NOT establish an87us
  critical section, ISR or even UART activity at the retained fault.

## Actual scheduling and instrumentation questions

adc_stream::interrupt processes a coherent scan, checks rawcurrent, queues
the originaltimestamp, records maxwall. Its priority0 equals TIM6 guard;
both preempt COMP/COM priority64. Do not call a100us guard PERIOD a100us ISR.
Existing maxwall summaries do not identify what interrupted a particular
COMP persistence window. Need aligned entry/exit/preemption evidence.

coretrace1 is NOT a passive switch: it enables per-read bookkeeping and skips
the specialized real/inverted/traceoff adapter. bench-irq-tail selects retained
tail storage but trace0 bypasses record production. Before retrying6.9% with
trace1, quantify instrumentation at the preceding qualified point, ensure
rawtrace/chronology/guard verification accepts the mode, and label it a changed
build/runtime sensing path. Never explain a new traced failure as E383's cause
without that comparison. Avoid higher duty/profile or widened cycle guards.

Next: inspect every masked section reachable from powered foreground plus
guard/DMA handlers, and verify tail records provide the chronology needed to
distinguish interrupt latency, persistence rejection and mux timing. If not,
add a bounded per-entry/exit tail instead of broad serial logging. Keep the
guard/timestamp/stack/observer-overhead limits intact. AM32 remains a reserve
comparison, not a prerequisite or an assumed successful result.
