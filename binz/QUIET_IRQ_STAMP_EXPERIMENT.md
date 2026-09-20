# Report-only comparator timestamp omission - E417

## E421 recovery passes6.8, late6.9 refusal remains

Same137F/start61, original30s campaigns. reentry68 PASS27.990639s resumed
315.395247eHz,52969COM/52968accepted,IRQ52.73825%,sigma23.730714us,
raw277bus10925,stack2676,seed1073/acquisition6749us/arm49cost12,DMA22queue2.
IRQ difference0.7917percentagepoints fromlatestA2DErecovery, n1comparison.
SHA23E2DB5AB0479C874E0CC2BA31407793571CBE02D583722C808E8C1247E365BB.

One subsequent reentry69 FAIL after27.133800s resumed319.662214eHz,
52042COM/52041accepted,sigma24.255099us,raw286bus10972,stack2676.
CycleTiming12 step1 previous27130743->27133760=3017us below3031floor.
Referencecycles6408/6029half-us ticks,pairedmean3109.25us; guard/reference
residual2.5us. Not independentrotorproof. SHA
D9E4339310691C9D493AE707DFA878475CEE4D89AC603353A98A2C011C0D7417.
Bothfinaloffverified/portsclosed. Nearlyfinished is notpass. No furtherduty
increase. Omitting reportstamp does noteliminate failure; no reliabilityclaim.

## E420 candidate boots and passes preceding hold

Actual137F installed after verifiedoff/noCOM41conflict. Reproduction boot
responded withoutrepair; quietstamp_boot_guard01PASS3faults/18refusals.
uart_candidate_before_repair01.jsonl (name means no repair, not failedboot)
matches workingbaseline afterguard:clock08040010,CR1d/CR2=CR3=0/BRR22b,
GPIOCmoderf3afffff/AFRH0,ISR006000d0,PD1zero/BDTRc1a/CCRs0.
PriorUARTfailure remainsintermittent/unexplained, not fixed or reproduced.
quietstamp_preflight02fivePASSCPU2/7/10.

quietstamp_start61_hold68_01 PASS10.000034s315.435361eHz18926COM/18926accepted,
IRQ52.825528%,raw274bus10901,stack3468,COMP40COM44commit21DMA22queue2,
sigma39.193us. IRQSTAMPomitted1, strictfixture/provenance/timeline/finaloffPASS.
SHA9C3EB731366CA39089BD1AEC087E7F06DA78C43EED4754FB04AEBE4817049510.
No convincing gain yet versus~53.5baseline from differentwindow. Next same
61startup/68BEMF original30s recoveryqualification; no higherdutyclaim.

## E418 emitted branch verified, UART qualification failed; baseline restored

137F ADC_COMP branch080019d8 bypasses masked timestamp block080019de..1a04
for powered trace0. Other accepted/stop clock work remains. Preflashoff and
no competingCOM41/probe verified. Candidate flashed/reset, guard01 UARTsilent.
ReadRCC08000000/PD1zero/BDTRc1a/CCRs0 then documented08040000repair.
guard02 and preflight01_pulse remainedsilent; subsequentread confirmed
RCC08040000 andsafeoutputs. No motor command or timing measurement.

Restored archivedA2DE. restore_guard01 UARTsilent; exactsafe registers checked
again, clockrepair, restore_guard02 PASS3faults/18refusals/finaloff. Portsclosed.
ActualA2DE, root137F notinstalled. Allcaptures prefixedquietstamp retained.
UARTcause unresolved; do not infer comparator timing regression from absent
shell response or repeat flash/repair without diagnosing boot/USART state.

Candidate137F0E1018F3E03B89920C79697A87335A369BE944C79FC943A183061B0CE940
release-s/LTO text123836/data1104/bss29336. NOTFLASHED; actualA2DE/offE415.

LAST_IRQ_US is written once per dispatched COMP call via observation_elapsed,
which takes a PRIMASK critical section and samples the extended TIM17 clock.
Its only consumer is the post-stop COAST_END_STATE report. It is not a guard,
COM scheduler, interval estimator, reference filter or acceptance input.

Optional bench-quiet-irq-stamp skips this report-only write on powered trace0
calls. Unpowered and trace-on diagnostics retain it. Observation reset zeros
the field, including recovery; IRQSTAMP explicitly reports omission so zero
must not be interpreted as a real timestamp. --quiet-irq-stamp requires exact
provenance and trace0. No comparator values, pending events or safety loads
are cached/skipped. Accepted-event and stop timestamps remain unchanged.

OBS_CLOCK must still be sampled inside each65.536ms TIM17 wrap. In powered
operation accepted callbacks feed it; missing accepted events trigger existing
independent1ms tracking stop, which samples it. This relies on the pre-existing
scheduling/guard bounds, not a new blackout guarantee. Preserve all guards.

No measured speedup yet. Generated-code audit must verify the powered branch
skips this particular update (other observation_elapsed calls remain valid),
then disabled preflights and bounded6.8 hold/recovery before higher-duty tests.
Do not omit other observation timestamps or weaken diagnostics silently.
