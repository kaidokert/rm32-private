//! Dump the BENCH factory-baseline EepromConfig as hex — mirrors the
//! `benchuart` persist block in rm32_stm32/src/bin/main.rs, including the
//! board's `bench_*` fields. Used to rebuild the bench EEPROM page when it
//! gets trashed (another project on the board, minz sessions).
//!
//!   cargo run -p rm32-bench --example dump_config -- l431|binz
//!
//! The board fields below mirror boards/neutron_l431.yaml and
//! boards/binz_drv8304h_g071.yaml; keep them in step. Leaving them out once
//! ran the L431 motor backwards on a variable carrier (2026-11-06).
//! Where to write the page: the L431 app reads 0x0800F800 when the bench
//! bootloader reports device code 0x35 (write 0x0801F800 too); binz has no
//! bootloader and reads 0x0800F800.
fn main() {
    let board = std::env::args().nth(1).unwrap_or_default();
    // (dir_reversed, pwm_khz, fixed_pwm, sine_start)
    let (dir_reversed, pwm_khz, fixed_pwm, sine_start) = match board.as_str() {
        "l431" => (true, 48, true, false),
        "binz" => (false, 48, true, true),
        _ => {
            eprintln!("usage: dump_config l431|binz");
            std::process::exit(2);
        }
    };
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
    c.dir_reversed = dir_reversed as u8;
    c.use_sine_start = sine_start as u8;
    if pwm_khz != 0 {
        c.pwm_frequency = pwm_khz;
    }
    if fixed_pwm {
        c.variable_pwm = 0;
    }
    if sine_start {
        c.sine_mode_changeover_throttle_level = 5;
        c.sine_mode_power = 3;
    }
    let b = c.as_bytes();
    let hex: Vec<String> = b.iter().map(|x| format!("{:02x}", x)).collect();
    println!("{}", hex.join(""));
}
