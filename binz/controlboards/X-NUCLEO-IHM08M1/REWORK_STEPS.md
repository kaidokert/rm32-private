# IHM08M1 rework — do these in order

## Parts to grab first
- 3 × 47 kΩ resistors
- thin wire, solder, flux, fine tip
- 1 × USB-TTL adapter (3.3 V)

## Steps (top to bottom)

1. [x] Desolder **R77** and remove it. ✅
2. [x] Remove the **SB16** 0 Ω resistor (frees PA2 from the ST-Link). ✅
3. [x] Remove the **SB18** 0 Ω resistor (frees PA3 from the ST-Link). ✅
4. [ ] Solder the three 47 kΩ resistors together at **one** end. Call that joined blob **STAR**.
5. [ ] Solder the other end of resistor #1 → **PC3**.
6. [ ] Solder the other end of resistor #2 → **PB11**.
7. [ ] Solder the other end of resistor #3 → **PB13**.
8. [ ] Wire **STAR → PA3**.
9. [ ] Wire **PC3 → PB3**.
10. [ ] Wire **PB11 → PB7**.
11. [ ] Wire **PB13 → PA2**.
12. [ ] Wire your throttle signal → **PB4**.
13. [ ] Wire USB-TTL: **RX → PC10**, **TX → PC11**, **GND → any GND**.

## Already done — nothing to do
- JP3 open ✅
- JP1, JP2 open ✅
- J5 / J6 single-shunt ✅

## Check with a meter (power OFF) before first boot
- [ ] PC3 connects to PB3. PB11 connects to PB7. PB13 connects to PA2.
- [ ] PA3 connects (through a resistor) to each of PC3, PB11, PB13.
- [ ] PA2 and PA3 no longer connect to the ST-Link.
- [ ] No new joint is shorted to GND or 3V3.
- [ ] No two neighbouring header pins touch.

## Do NOT remove
R59, R60, R62, R81 — leave them alone.

## When soldering is done
Flash the `x_nucleo_ihm08m1_g071` board def (no code changes), then power up.
