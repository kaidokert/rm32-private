<entry> ADC1_COMP_IRQHandler
8004ef4 T      # falling EXTI18 flag set
8004f3c T      # INTERVAL_TIMER->CNT > average_interval>>1: accept path
8004f50 call   # interruptRoutine()
8000e92 N      # filter_level != 0
8000ea4 call   # getCompOutputLevel()
8000eac T      # read != rising: keep filtering (== rising would return)
8000ea2 N*2    # i < filter_level(3 at ci~65us, map(130,100,500,3,12)): 3 reads then exit
8000eb2 call   # maskPhaseInterrupts()
