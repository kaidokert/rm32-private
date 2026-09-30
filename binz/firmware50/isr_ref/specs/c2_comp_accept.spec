<entry> ADC_COMP
800130e T   # det active
800131e T   # step < 6
8001326 N   # step != 0
800133e T   # count > gate (open)
8001350 T   # depth < 12
8001356 N   # depth != 0
800136c T   # read == post-crossing
8001372 T*2 # AM32 depth 3 at 72.5-75 %
80013a2 N   # com.active: arm
80013c8 N   # active
80013cc N   # not stopped
80013d8 T   # left no underflow
80013e2 T   # left > 1
80013ec N   # us <= 0xFFFF
80013fc T   # us >= 2
8001434 T   # not late
80015ae T   # same bucket (report-only limiter)
80015c2 T   # no saturation
80015d6 T   # count not > peak
80015e2 T   # no storm injection pending
8001600 T   # no overrun
