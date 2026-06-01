# B-G431B-ESC1 Pinout — STM32G431CB Motor Control

Source: UM2516 Rev 4 (ST user manual for B-G431B-ESC1 Discovery kit)

## CAN Termination (Table 3)


## STM32G431CB Pin Assignment (Table 4)

| Pin | MCU Pin | Signal | Notes |
|---|---|---|---|

| 9 | PA1 | Curr_fdbk1_OPAmp+ | |
| 11 | PA3 | Curr_fdbk1_OPAmp− | |
| 13 | PA5 | Curr_fdbk2_OPAmp− | |
| 15 | PA7 | Curr_fdbk2_OPAmp+ | |
| 17 | PB0 | Curr_fdbk3_OPAmp+ | |
| 19 | PB2 | Curr_fdbk3_OPAmp− | |

| 12 | PA4 | BEMF1 | |
| 16 | PC4 | BEMF2 | |
| 24 | PB11 | BEMF3 | |

| 10 | PA2 | OP1_OUT | Opamp 1 output |
| 14 | PA6 | OP2_OUT | Opamp 2 output |

| 38 | PA15 | PWM | Input signal (J3), 5 V tolerant | - actual external controller input
| 39 | PC10 | BUTTON | On daughterboard | - yet to be used
| 43 | PB5 | GPIO_BEMF | BEMF enable/select |

| 30 | PA8 | TIM1_CH1 | Motor PWM phase A(U) high |
| 31 | PA9 | TIM1_CH2 | Motor PWM phase B(V) high |
| 32 | PA10 | TIM1_CH3 | Motor PWM phase C(W) high |
| 34 | PA12 | TIM1_CH2N | Motor PWM phase B(V) low | - clocked
| 28 | PB15 | TIM1_CH3N | Motor PWM phase C(W) low |
| 2  | PC13/TAMP/RTC | TIM1_CH1N | Motor PWM phase A(U) low |

huh ?
| 44 | PB6 | A+/H1 | Motor phase A low / Hall 1 |
| 45 | PB7 | B+/H2 | Motor phase B low / Hall 2 |
| 46 | PB8 | Z+/H3 | Motor phase C low / Hall 3 |

| 18 | PB1 | TP3 | Test point |


## PWM Input Signal (J3)

- Frequency: 490 Hz
- Min speed: 1060 µs pulse
- Max speed: 1860 µs pulse
- Below 1060 µs: ESC not armed
- Blank >1500 ms while running: ESC off
