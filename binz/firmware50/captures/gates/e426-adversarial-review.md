No candidate-specific software blocker is demonstrated by the supplied source and assembly. The fresh dependency is real: the clamped blend is stored at `0x0800096a`, transferred into the wait calculation, and used for the deadline check and timer reload. This is not merely a renamed policy.

One unresolved deadline issue: `spent` is sampled at `0x080009f0`, but TIM16 starts at `0x08000a86`. That intervening time is unaccounted for; a positive one-microsecond remainder also receives a two-microsecond reload. The exhausted-wait check therefore does **not** prove commutation meets the original deadline. This is not established as a fresh-only regression.

Fresh scheduling shortens waits during contraction and lengthens them during rebound. The frozen replay establishes arithmetic behavior, not improved physical tracking or survival.

Observer cost remains material: OrderRing writes occur after arming, and watchdog processing adds another interrupt-masked interval. Static entry counts and the clean ISR audit establish neither WCET nor timer-service latency.

The reported tests support functional consistency. They do not resolve timing margin; treat the proposed 15% run as an experiment, with 25% contingent on reviewing its evidence.
