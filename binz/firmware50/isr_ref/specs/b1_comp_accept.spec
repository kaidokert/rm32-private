<entry> ADC_COMP
80012ae N   # det active
80012ba N   # cap armed
80012c2 T   # not failed
80012d2 N   # same bucket
8001416 T   # no saturation
800142a T   # count not > peak
8001432 N   # count <= LIMIT
8001438 T   # within
800144c T   # zc Some
800145a T   # no rebase
800146c T   # step clamp (as A6)
8001476 N
8001480 T   # avg fits i32 / bool
8001514 T   # avg >= SHALLOW
800151c T   # > MAP_IN_LOW
8001526 T   # < MAP_IN_HIGH
8001562 N   # floor 5
800156c T   # count > gate
8001590 N   # read == expected
8001598 T*4 # 5 reads
80015a8 N   # com.active: arm
80015ca N   # active
80015ce N   # not stopped
80015d6 T   # left no underflow
80015de T   # left > 1
80015e8 N   # us <= 0xFFFF
80015f4 T   # us >= 2
8001628 T   # not late
800164e T   # tracking on
8001670 T   # 2*avg < 667
80016fe N   # floor 200
800170a N
800170e T
800171c N   # fault None
8001734 T   # not stale
80017c0 N   # prev Some
80017cc N   # prev != 6
80017d6 N   # in order
80017e6 N   # sector != 0
80017ec N
80017f4 N
80017fc T   # no saturation
800180a T   # gap >= min
80017b6 T   # no overrun
