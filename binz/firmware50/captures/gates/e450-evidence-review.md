The proposed fix reaches this binary’s plain COMP refusal and DiodeLatched COM paths, but leaves concrete gaps:

- **Ownership:** `arm_allowed` proves run eligibility, not permission to resume this sector. Under `com-top`, COM can preempt COMP’s decision, establish a blank, then the older refused COMP resumes and unmasks during that blank. Check sector/blank ownership atomically too.
- **Pending semantics:** Refusal preserves edges latched during the handler; phase 3 preserves blank-window edges. Enabling can dispatch immediately. A stop check neither validates their sector nor timestamps them.
- **Coverage:** Public foreground/acquisition `comp_exti_arm` and driven refusal remain unguarded. A guard stop followed by either continuation can re-enable the line; the unconditional “stop before/between/after” guarantee therefore exceeds coverage.
- **Stop definition:** `com_stop()` latches and stops TIM16 but does not disable COMP. The guarantee holds only for shutdown paths that also mask COMP.

The masked check-and-enable closes the local guard interleaving; it does not establish global resume ownership or complete stop coverage.
