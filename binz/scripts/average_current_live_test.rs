use std::sync::atomic::{AtomicU32,AtomicI32,Ordering::Relaxed};
static STATE:AtomicU32=AtomicU32::new(8);
static ZERO:AtomicI32=AtomicI32::new(-1);
fn get_idr(_port:u8,_pin:u8)->bool {STATE.load(Relaxed)&1!=0}
mod cortex_m {pub mod interrupt {pub fn free<R>(f:impl FnOnce(&())->R)->R {f(&())}}}
mod powered_timer {
    pub fn outputs_disabled()->bool {super::STATE.load(super::Relaxed)&8!=0}
    pub fn owns()->bool {super::STATE.load(super::Relaxed)&2!=0}
}
mod core_bench {pub fn active()->bool {super::STATE.load(super::Relaxed)&4!=0}}
mod prestart_baseline {
    pub fn current_zeros()->Option<(i32,[u16;3])> {
        let z=super::ZERO.load(super::Relaxed);(z>=0).then_some((z,[2048;3]))
    }
}
#[path="../examples/support/average_current_live.rs"] mod actual;
#[test] fn actual_adapter_refuses_missing_config_and_revokes_on_shutdown() {
    assert!(!actual::install());assert!(!actual::scan([2048;3]));
    for state in [1,2,4,0] {STATE.store(state,Relaxed);assert!(!actual::configure(50));}
    STATE.store(8,Relaxed);
    assert!(!actual::configure(0));assert!(!actual::configure(614251));
    assert!(actual::configure(50));assert!(!actual::install());
    ZERO.store(6144*50,Relaxed);assert!(actual::install());
    assert!(actual::scan_raw([2048;3],1500,1500));
    // 1774 raw-count ratio is the exact accepted boundary; one more fails.
    assert!(actual::scan_raw([2048+1774,2048,2048],1500,1500));
    assert!(!actual::scan_raw([2048+1775,2048,2048],1500,1500));
    actual::revoke();assert!(actual::install());
    for _ in 0..50 {assert!(actual::scan([2047,2048,2048]));}
    // Producer changes do not reinstall the zero or discard a pending block.
    // This tests sample-count continuity, not ADC cadence/physical calibration.
    for _ in 0..50 {assert!(actual::scan([2046,2048,2048]));}
    for _ in 0..49 {assert!(actual::scan([2046,2048,2048]));}
    assert!(!actual::scan([2046,2048,2048]));
    assert!(!actual::scan([2048;3]));
    actual::revoke();assert!(!actual::scan([2048;3]));
    ZERO.store(6147*50,Relaxed);assert!(actual::install());
    for _ in 0..50 {assert!(actual::scan([2049;3]));}
    assert!(!actual::scan([4095,2048,2048]));
    actual::revoke();ZERO.store(-1,Relaxed);assert!(!actual::install());
}
