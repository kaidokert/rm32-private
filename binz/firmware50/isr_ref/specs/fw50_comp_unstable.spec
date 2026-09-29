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
80015e0 N   # first read dissents -> Unstable
800189a T
80018a0 T
80018aa N   # refused: resume the line
80018cc N
80018d0 N
80018d4 N
80018d8 N
80018dc T   # resume allowed
