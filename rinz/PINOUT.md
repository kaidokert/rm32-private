# B-G431B-ESC1 Pinout — STM32G431CB Motor Control

Source: UM2516 Rev 4 (ST user manual for B-G431B-ESC1 Discovery kit)

## CAN Termination (Table 3)

| CAN_TERM pin | 120 Ω resistor |
|---|---|
| H | ON |
| L | OFF (high impedance) |

## STM32G431CB Pin Assignment (Table 4)

| Pin | MCU Pin | Signal | Notes |
|---|---|---|---|
| 1 | VBAT | 3V3 | |
| 2 | PC13/TAMP/RTC | TIM1_CH1N | |
| 3 | PC14 | CAN_TERM | Solder bridge R26 |
| 4 | PC15 | N.C. | |
| 5 | PF0/OSC-IN | OSC 8 MHz | |
| 6 | PF1/OSC-OUT | OSC 8 MHz | Solder bridge R27 |
| 7 | PG10/NRST | RESET | |
| 8 | PA0 | VBUS | |
| 9 | PA1 | Curr_fdbk1_OPAmp+ | |
| 10 | PA2 | OP1_OUT | Opamp 1 output |
| 11 | PA3 | Curr_fdbk1_OPAmp− | |
| 12 | PA4 | BEMF1 | |
| 13 | PA5 | Curr_fdbk2_OPAmp− | |
| 14 | PA6 | OP2_OUT | Opamp 2 output |
| 15 | PA7 | Curr_fdbk2_OPAmp+ | |
| 16 | PC4 | BEMF2 | |
| 17 | PB0 | Curr_fdbk3_OPAmp+ | |
| 18 | PB1 | TP3 | Test point |
| 19 | PB2 | Curr_fdbk3_OPAmp− | |
| 20 | VREF+ | 3V3 | |
| 21 | VDDA | 3V3 | |
| 22 | PB10 | N.C. | |
| 23 | VDD4 | 3V3 | |
| 24 | PB11 | BEMF3 | |
| 25 | PB12 | POTENTIOMETER | On daughterboard |
| 26 | PB13 | N.C. | |
| 27 | PB14 | Temperature feedback | NTC |
| 28 | PB15 | TIM1_CH3N | |
| 29 | PC6 | STATUS | LED |
| 30 | PA8 | TIM1_CH1 | Motor PWM phase A high |
| 31 | PA9 | TIM1_CH2 | Motor PWM phase B high |
| 32 | PA10 | TIM1_CH3 | Motor PWM phase C high |
| 33 | PA11 | CAN_RX | |
| 34 | PA12 | TIM1_CH2N | Motor PWM phase B low |
| 35 | VDD6 | 3V3 | |
| 36 | PA13 | SWDIO | |
| 37 | PA14 | SWCLK | |
| 38 | PA15 | PWM | Input signal (J3), 5 V tolerant |
| 39 | PC10 | BUTTON | On daughterboard |
| 40 | PC11 | CAN_SHDN, TP2 | CAN shutdown + test point |
| 41 | PB3 | USART2_TX | J3 UART TX (telemetry out) |
| 42 | PB4 | USART2_RX | J3 UART RX (firmware update) |
| 43 | PB5 | GPIO_BEMF | BEMF enable/select |
| 44 | PB6 | A+/H1 | Motor phase A low / Hall 1 |
| 45 | PB7 | B+/H2 | Motor phase B low / Hall 2 |
| 46 | PB8 | Z+/H3 | Motor phase C low / Hall 3 |
| 47 | PB9 | CAN_TX | |
| 48 | VDD8 | 3V3 | |

## External Connectors

| Connector | Function |
|---|---|
| J3 | PWM input (PA15, 5 V tolerant), UART TX (PB3), UART RX (PB4), GND, 5 V BEC out (daughterboard only) |
| J5/J6 | LiPo battery power input, 3S–6S |
| J7 | 3-phase motor (U, V, W) |
| J8 | Motor sensor — Hall (H1/H2/H3 = PB6/PB7/PB8) or encoder, +5 V, GND |
| J1 | CAN bus (PA11 RX, PB9 TX), +5 V in/out, GND |

## SWD Without Daughterboard (J4 pads, Table 6)

| J4 pad | Signal |
|---|---|
| 1 | SWDIO (PA13) |
| 2 | SWCLK (PA14) |
| 3 | VDD (3V3) |
| 4 | GND |

## PWM Input Signal (J3)

- Frequency: 490 Hz
- Min speed: 1060 µs pulse
- Max speed: 1860 µs pulse
- Below 1060 µs: ESC not armed
- Blank >1500 ms while running: ESC off
