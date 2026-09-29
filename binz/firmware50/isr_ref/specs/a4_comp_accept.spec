<entry> ADC_COMP
8001316 N   # det.active
8001320 N   # cap_armed
8001328 T   # rate.hit: not failed
800133c N   # same 1 ms bucket
8001482 T   # no saturation
8001496 T   # count <= peak
800149e N   # count <= LIMIT
80014a8 T   # within the storm limit
80014c2 T   # no rebase
80014e6 T   # step 3 < 6
80014f2 N   # step != 0
8001500 T   # count > published gate
800150c N   # depth != 0
8001528 T   # read == expected
8001530 T*4 # 5 reads
8001556 N   # com.active: arm
800156c T   # left: no underflow
8001582 N   # com.active
8001586 N   # not stopped
8001590 T   # left > 1
800159a N   # us <= 0xFFFF
80015a0 T   # us >= 2
80015d6 T   # no carry
80015e4 T   # ci not a new min
80015ec T   # left > 2
800160e T   # spent <= spent_max
800161c N   # tracking on
8001622 N   # average fits i32
8001790 T   # 2*avg < 667
80017a2 N   # floor 200
80017b0 N   # >= min
80017b4 T   # no store
80017c4 N   # fault None
80017dc N   # not stale
80017e4 N   # prev Some
80017ec N   # prev != 6
8001808 N   # sector != 0
8001810 N   # in order
800181a T   # gap >= min
8001702 T   # elapsed <= call_max
8001708 T   # no overrun
8001712 T   # accepted: return
