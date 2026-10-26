//! Dump the BENCH factory-baseline EepromConfig as hex — mirrors the
//! `benchuart` persist block in rm32_stm32/src/bin/main.rs. Used to
//! rebuild the bench EEPROM page when it gets trashed (minz sessions).
fn main() {
    let mut c = rm32::config::EepromConfig::default();
    c.apply_version_defaults();
    c.reserved_0 = 1; // bootloader jump-enable flag
    c.version_major = 2;
    c.version_minor = 20;
    c.comp_pwm = 1;
    c.variable_pwm = 1;
    c.advance_level = 26;
    c.temperature_limit = 141;
    c.motor_kv = 55;
    c.motor_poles = 14;
    c.minimum_duty_cycle = 4;
    c.startup_power = 105;
    c.beep_volume = 2; // operator preference: quiet
    let b = c.as_bytes();
    let hex: Vec<String> = b.iter().map(|x| format!("{:02x}", x)).collect();
    println!("{}", hex.join(""));
}
