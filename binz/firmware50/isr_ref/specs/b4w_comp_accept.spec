<entry> ADC_COMP
800130c T   # det active
800131a T   # step < 6
8001322 N   # step != 0
8001336 T   # count > gate (open)
800134a T   # depth < 12
8001350 N   # depth != 0
8001366 T   # read == post-crossing
800136c T*2 # AM32 depth 3 at 72.5 % (am32_map(120,100,500,3,12) = 3)
8001398 N   # com.active: arm
80013c2 N   # active
80013c6 N   # not stopped
80013d4 T   # left no underflow
80013de T   # left > 1
80013ea N   # us <= 0xFFFF
80013fa T   # us >= 2
8001432 T   # not late
80015ac T   # same bucket (report-only limiter)
80015be T   # no saturation
80015ce T   # count not > peak
80015e4 T   # elapsed < 51: no overrun
