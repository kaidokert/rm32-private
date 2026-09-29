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
80014be N   # first read dissents -> Unstable
8001696 T
800169c T
80016a6 N   # refused: resume the line
80016c8 N
80016cc N
80016d0 N
80016d4 N
80016d8 T   # resume allowed
