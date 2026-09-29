<entry> ADC_COMP
80012ae N
80012b8 N
80012c0 T
80012d4 N
800141a T
800142c T
8001438 N
8001442 T
8001456 T   # zc Some (tag)
8001464 N   # zc Some
800146e T   # no rebase
8001496 T   # step 3 < 6
80014a0 N   # step != 0
80014b0 N   # avg fits i32
80014ba T   # avg >= SHALLOW
8001546 T   # > MAP_IN_LOW
8001554 T   # < MAP_IN_HIGH
80015a2 N   # floor 5
80015b0 T   # count > gate
80015d4 T   # read == expected
80015de T*4 # 5 reads
80015f2 N   # com.active: arm
8001602 T   # left no underflow
800161a N
800161e N
8001626 T   # left > 1
8001630 N   # us <= 0xFFFF
8001638 T   # us >= 2
8001676 T   # no carry
8001682 T   # ci not new min
8001688 T   # left > 2
8001692 T   # not late
80016a8 T   # spent <= max
80016cc N   # tracking on
80016d2 T   # avg fits i32
800179e T   # 2*avg < 667
80017b0 N   # floor 200
80017bc N
80017c0 T
80017d0 N   # fault None
80017e8 T   # not stale
8001866 N   # prev Some
8001870 N   # prev != 6
8001888 N   # sector != 0
8001890 N   # in order
8001898 T   # gap >= min
8001702 T
8001708 T
8001712 T   # accepted: return
