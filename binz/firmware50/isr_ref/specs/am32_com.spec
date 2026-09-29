<entry> TIM14_IRQHandler
8004fda call   # PeriodElapsedCallback()
8000e00 call   # commutate()
80009c4 T      # forward == 1
8000a48 T      # step+1 <= 6: no wrap
80009e4 T      # comp_pwm/prop_brake flag == 0 -> comStep(step)
8000a3a call   # comStep(step)
8005638 ->800563a  # jump table: case 1 (representative step)
800568e T      # step-1 flag byte == 0 (non-complementary branch skipped)
80009e8 call   # changeCompInput()
8005630 N      # step <= 6
8005408 N      # average_interval < 400 (high speed)
8005410 N      # medium_speed_set == 0 already
800541c T      # average_interval <= 600
8005438 T      # step == 1 (c floating)
8005440 N      # step != 2
8005444 N      # step != 5
8005448 N      # step != 3
800544c N      # step != 6
8005454 T      # rising == 0 for this step: falling-edge EXTI config
80009f8 T      # average_interval <= changeover+500: stay interrupt mode
8000e24 N      # auto_advance off: fixed advance
8000e4a T      # old_routine == 0 -> enableCompInterrupts
8000e6a call   # enableCompInterrupts()
8000e52 T      # zero_crosses >= 10000 (steady state): no increment
