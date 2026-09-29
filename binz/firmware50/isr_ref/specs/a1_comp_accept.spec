<entry> ADC_COMP
80012ae N   # det.active
80012b8 N   # cap_armed (steady state)
80012c0 T   # rate.hit: not failed
80012d4 N   # observe: same 1 ms bucket
800141a T   # saturating_add: no saturation
800142e T   # count <= peak
8001436 N   # count <= LIMIT
8001440 T   # within the storm limit
800145a T   # ci_max >= count: no rebase
800147e T   # step 3 (< 6): no clamp
800148a N   # step != 0
8001496 T   # count > published gate
80014a2 N   # depth != 0
80014be T   # read == expected level
80014c6 T*4 # 5 reads (published depth 5)
80014ec N   # com.active: arm path
8001502 T   # left = wait - spent: no underflow
8001518 N   # com.active
800151c N   # not stopped
8001526 T   # left.max(1): left > 1
8001530 N   # us <= 0xFFFF
8001536 T   # us >= 2
800156c T   # ci_p1 saturating_add: no carry
800157a T   # ci not a new minimum
8001582 T   # left > 2: not thin
80015a4 T   # spent <= spent_max
80015b2 N   # guard tracking on
80015b8 N   # average fits i32
8001724 T   # 2*avg (130) < 667
8001736 N   # 195 <= 200 -> floor 200
8001744 N   # 200 >= min_interval
8001748 T   # 200 >= current max: no store
8001758 N   # fault is None
8001770 N   # not stale
8001778 N   # previous sector is Some
8001780 N   # prev != 6
8001696 T   # elapsed <= call_max
800169c T   # elapsed <= 50: no overrun
80016a6 T   # accepted: return
800179c N   # sector != 0
80017a4 N   # in order
80017ae T   # gap >= min_interval
