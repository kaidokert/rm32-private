<entry> TIM16
80019be N   # com.active
80019cc N   # late < 0x8000
80019d4 T   # late <= late_max: no store
80019dc T   # phase 1: commutation
8001a6a T   # previous step 2 < 6
8001a74 N   # step != 0
8001a7a N   # 2 <= 5: next = 3
8001a96 N   # plan is Some: apply
8001afc T   # six-slot index < 7
8001b14 T   # interval < 1e6: no clamp
8001b26 T   # reverse blank not due
8001bce T   # blanking - since: no underflow
8001bd4 N   # hold >= BLANK_ARM_MIN_US (16): arm the floor (phase 3)
8001c36 N   # com.active
8001c3a N   # not stopped
8001c58 T   # us <= 0xFFFF
