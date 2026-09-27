Raw: plan publication and preload staging share an IRQ mask; UDIS spans compare/ARR writes and publication. Releasing UDIS permits a native update before interrupts resume. The reported 26.312 µs is conditional instruction accounting, not a measured maximum. The excerpt does not establish hardware fault shutdown timing or current-limit response.

Concrete exposure: during that mask, pending COM and software protection handlers cannot execute. The bridge can retain the previous sector beyond its intended deadline; late-arm/tracking checks can stop afterward but cannot undo that interval. “Stops stay active” needs this qualification.

I would admit **one bounded screen as specified**, after exact-image verification and passing all-off bootchecks, with the physical 3 A limit enforced, propless setup secured, independent power removal available, and no retry after any fault. The supplied sequence exposes no demonstrated mixed-publication race; it does leave commutation-delay risk unquantified. Hours off suggests cooling but does not establish temperature.

A pass supports only this run’s outcome—not thermal safety, timing qualification, or a causal improvement over entry48k.