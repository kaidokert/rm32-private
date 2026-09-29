<entry> ADC_COMP
80012ae N   # det.active
80012ba N   # cap_armed (steady state)
80012c4 T   # rate.hit: not failed
80012d8 N   # observe: same 1 ms bucket (common case)
8001426 T   # saturating_add: no saturation
800143a T   # count <= peak (common)
8001442 N   # count <= LIMIT: not failed
800144e T   # within the storm limit
8001464 N   # zc is Some
800146e T   # count <= ci_max: no rebase
800149c T   # step 3 (< 6): no clamp
80014a6 N   # step != 0
80014b8 N   # count > blanking: past the half-cycle gate
80014c2 T   # average_interval fits i32 (always)
800155e T   # avg (130 half-us at 72.5%, ci ~65 us) >= SHALLOW 50
8001566 T   # avg > MAP_IN_LOW (130 > 100)
8001570 T   # avg < MAP_IN_HIGH
80015be N   # mapped add 0 -> floor path: depth = 5 (deep-filter)
80015e0 T   # read == expected level: continue filtering
80015ea T*4  # i < depth(5): 5 reads then exit
8001606 T   # blend saturating_add: no overflow
8001614 T   # blend saturating_add: no carry
8001620 T   # clamp: v < max_interval
8001626 N   # clamp: v >= min_interval
8001650 N   # com.active: arm path
8001656 T   # advance level 18 < 64
800166c T   # wait computation: no underflow
800167c T   # left = wait - spent > 0 (on-time arm)
8001694 N   # com.active
8001698 N   # not stopped: arm allowed
800169e T   # left.max(1): left > 1
80016a8 N   # us <= 0xFFFF
80016b0 T   # us >= 2
80016ea T   # ci_p1 saturating_add: no carry
80016f8 T   # ci not a new minimum
80016fe T   # left > 2: not thin
800170a T   # left > 0: not late
8001720 T   # spent <= spent_max
8001730 T   # guard tracking on (closed loop)
8001736 T   # average fits i32
8001768 T   # 2*avg (130) < 667
800177a N   # (130*3+1)/2 = 195 <= 200 -> floor 200
8001784 N   # 200 >= min_interval
8001788 T   # 200 >= current max (already tightened): no store
8001798 N   # fault is None (niche 3)
80017ac T   # gap <= max_interval: not stale
80017ba N   # previous sector is Some
80017c4 N   # prev != 6
80017dc N   # sector != 0
80017e6 N   # sector == prev + 1: in order
80017f0 T   # gap >= min_interval: not too fast
800189a T   # elapsed <= call_max
80018a0 T   # elapsed <= 50 us: no overrun
80018aa T   # accepted: stay masked, return
