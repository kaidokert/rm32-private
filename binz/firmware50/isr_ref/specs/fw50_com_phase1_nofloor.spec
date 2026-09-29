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
8001bd4 T   # hold < 16: no floor, arm the line now (comp_exti_arm)
8001c7a call   # comp_exti_arm(step)
8000d92 N   # guard == 0
8000d96 N   # detector owns the line
8000d9a N   # active
8000d9e N   # not stopped
8000da2 T   # phase == 0: resume allowed
