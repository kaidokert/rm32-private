<entry> ADC_COMP
80012a2 T   # det active
80012b0 T   # step < 6
80012b8 N   # step != 0
80012ce T   # count > gate (AM32 gate open)
80012e0 T   # depth < 12
80012e6 N   # depth != 0
80012fc T   # read == post-crossing
8001302 T*2 # AM32 depth 3 at 72.5 % (map(120,100,500,3,12) = 3)
800132e N   # com.active: arm
8001352 N   # active
8001356 N   # not stopped
8001364 T   # left no underflow
800136e T   # left > 1
800137a N   # us <= 0xFFFF
8001386 T   # us >= 2
80013b8 T   # not late
8001438 T   # same bucket (report-only limiter)
800144a T   # no saturation
800145e T   # count not > peak
8001472 N   # no overrun
