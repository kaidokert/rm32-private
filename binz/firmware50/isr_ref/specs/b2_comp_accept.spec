<entry> ADC_COMP
80012b2 N   # det active
80012c6 T   # same bucket (report-only limiter: observe)
80012fa T   # no saturation
800130c T   # count not > peak
8001316 T   # zc Some
8001326 T   # no rebase
8001338 T   # step < 6
8001342 N   # step != 0
800134c T   # avg fits i32
800142c T   # avg >= SHALLOW
8001434 T   # > MAP_IN_LOW
80014c2 T   # < MAP_IN_HIGH
80014d6 N   # floor 5
80014e0 T   # count > gate
8001502 N   # read == expected
800150a T*4 # 5 reads
800151c N   # com.active: arm
800153c N   # active
8001540 N   # not stopped
800154c T   # left no underflow
8001554 T   # left > 1
800155e N   # us <= 0xFFFF
800156c T   # us >= 2
80015a0 T   # not late
80015d2 N   # no overrun
