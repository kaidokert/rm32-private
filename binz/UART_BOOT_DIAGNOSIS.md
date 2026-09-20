# UART boot diagnosis - E419

Actual archivedA2DE baseline remains installed, outputs disabled. No firmware
change or flash this entry. Prior candidate137F UART silence is unresolved.

Source shell-pwm enables USART3 APBENR1 bit18, reads back and DSBs before
HAL construction. HAL BasicConfig constructor enables it again, writes BRR,
configures control/pins and enables UE. Generated candidate code contains
both clock writes before BRR. No later boot write clearing this bit has been
identified; do not label the root cause proven or merely add another enable.

Prior failed captures only had RCC and bridge-safe values. They cannot tell
whether UE/BRR or pin mux setup completed. New drv_uart_snapshot.py collects
read-only RCC, GPIOC, USART control/status and bridge registers via serialized
probe commands. It deliberately excludes RDR/ICR/TDR; no UART byte consumption,
flag clearing, clock repair or motor command. Use only with no competing probe
or serial fixture. JSONL is exclusive-created and retains partial failures.

Working baseline captures/uart_baseline_snapshot01.jsonl:

- APBENR1=08040010, CCIPR=0.
- GPIOC MODER=f3afffff, AFRH=0.
- USART3 CR1=0000000d, CR2=0, CR3=0, BRR=0000022b.
- USART3 ISR=006000d0.
- ENABLE ODR=0, TIM1 BDTR=00000c1a, CCR1/2/3=0.

The snapshot was taken after disabled guard tests, so TIM6 clock bit4 is
expected extra state, not a required reset value. Peripheral addresses and
USART3 alias to USART1 register layout verified against cachedG071PAC.
Next obtain candidate snapshot immediately after failed shell response and
before repair. Compare control/baud/pins, not just clock enable. No fresh
candidate flash or repeated repair performed in this entry.
