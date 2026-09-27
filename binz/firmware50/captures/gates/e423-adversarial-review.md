**Preflight remains unverified:** `p` prints a constant PASS line after `controller.command`; it neither reruns these checks nor directly verifies ENABLE-low or pad state. Reaching the command loop establishes that the four boot predicates passed, but their implementations and actual boot results are absent. If this banner is the planned evidence for disabled pad verification, that is a concrete evidence gap before command `9`.

The shown recorder has separate COMP/COM storage. TIM16 disassembly confirms the latch write precedes two TIM2 reads bracketing TIM17. That bracket locates the **post-application timestamp**, not the latch event or physical switching edge; preemption before its first read is outside its reported uncertainty.

`OrderSag::freeze()` disables recording, but this excerpt does not establish that `Production` uses `OrderSag`. Main’s disarm proves freezing after command return, not at the stop decision. Any stop-tail interpretation needs that wiring verified.

The supplied tests/audit support limited software checks; they do not establish hardware preflight, image identity, or timing margins. I find no demonstrated ring-ownership defect here, but insufficient evidence to clear those preflight and freeze claims.
