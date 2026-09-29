<entry> TIM16
80018fe N   # com.active
800190c N   # late < 0x8000
8001914 T   # late <= late_max
800191c T   # phase 1
80019a8 T   # previous step 2 < 6
80019b2 N   # step != 0
80019ba N   # next = 3
80019d8 N   # plan Some
8001a42 N   # new acceptance: commit
8001a52 N   # zc is Some
8001a68 T   # blend: no overflow
8001a74 T   # blend: no carry
8001a84 T   # clamp: v < max
8001a90 N   # clamp: v >= min
8001aa4 T   # advance level < 64
8001ab8 T   # wait: no underflow
8001aca T   # average fits i32 (FromMicros shift path)
8001af8 T   # avg (130 half-us) >= SHALLOW 50
8001b00 T   # avg > MAP_IN_LOW
8001b0e T   # avg < MAP_IN_HIGH
8001b22 N   # mapped add 0 -> floor: depth 5
8001b34 T   # six-slot index in range
8001b48 T   # interval < 1e6
8001b5a T   # reverse blank not due
8001bdc T   # blanking - since: no underflow
8001be2 N   # hold >= 16: arm the floor
8001c46 N   # com.active
8001c4a N   # not stopped
8001c68 T   # us <= 0xFFFF
