<entry> ADC_COMP
80012ae N   # det.active
80012b8 N   # cap_armed
80012c0 T   # not failed
80012d4 N   # same bucket
8001418 T   # no saturation
800142a T   # count <= peak
8001436 N   # <= LIMIT
8001440 T   # within storm limit
800146c T   # no rebase
8001492 T   # step 3 < 6
800149c N   # step != 0
80014ba T   # avg >= SHALLOW
8001546 T   # avg > MAP_IN_LOW
8001556 T   # avg < MAP_IN_HIGH
80015a2 N   # floor 5
80015b4 T   # count > gate
80015dc T   # read == expected
80015e6 T*4 # 5 reads
80015fc N   # com.active: arm
800160c T   # left: no underflow
8001624 N   # com.active
8001628 N   # not stopped
8001630 T   # left > 1
800163a N   # us <= 0xFFFF
8001642 T   # us >= 2
8001680 T   # no carry
800168c T   # ci not new min
8001694 T   # left > 2
80016b6 T   # spent <= spent_max
80016ce T   # blend no overflow
80016d8 T   # blend no carry
80016e4 T   # clamp v < max
80016ec N   # clamp v >= min
80016f8 T   # level < 64
800170c T   # wait no underflow
8001730 N   # tracking on
8001736 T   # avg fits i32
80017fe T   # 2*avg < 667
8001810 N   # floor 200
800181c N   # >= min
8001820 T   # no store
800182c N   # fault None
8001840 T   # not stale
80018c0 N   # prev Some
80018c8 N   # prev != 6
80018e0 N   # sector != 0
80018e6 N   # in order
8001764 T   # elapsed <= call_max
800176a T   # no overrun
8001774 T   # accepted: return
8001454 T   # zc tag != 0: Some
8001462 N   # zc is Some
80014b0 N   # avg fits i32
800169e T   # left > 0: not late
80018ee T   # gap >= min_interval
