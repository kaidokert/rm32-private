<entry> ADC_COMP
80012a2 T   # det active
80012b0 T   # step < 6
80012b8 N   # step != 0
80012ce T   # count > gate (AM32 gate open)
80012e0 T   # depth < 12
80012e6 N   # depth != 0
80012fc T   # read == post-crossing
8001302 T*2 # AM32 depth 3 at 72.5 % (map(120,100,500,3,12) = 3)
800132c N   # com.active: arm
8001352 N   # active
8001356 N   # not stopped
8001360 T   # left no underflow
800136a T   # left > 1
8001374 N   # us <= 0xFFFF
8001382 T   # us >= 2
80013b8 T   # not late
800143a T   # same bucket (report-only limiter)
800144c T   # no saturation
8001460 T   # count not > peak
8001474 N   # no overrun
