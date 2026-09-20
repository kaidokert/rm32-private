#[path = "../examples/support/accepted_timing.rs"]
mod accepted_timing;
#[path = "../examples/support/powered_guard.rs"]
mod powered_guard;

use powered_guard::{speed_event_limit_us, Fault, Feedback, RunGuard};

fn sample() -> Feedback {
    Feedback {
        phase: [2048; 3],
        bus_mv: 11_700,
        vref: 1500,
    }
}

fn main() {
    let mut previous = speed_event_limit_us(64);
    for interval in 64..=100_000 {
        let limit = speed_event_limit_us(interval);
        assert!((200..=1000).contains(&limit));
        assert!(limit >= previous);
        previous = limit;
    }
    type Guard = RunGuard<2223, 100, true>;
    let mut guard = Guard::with_limits(0, 0, 1, sample(), 100_000, 20_000).unwrap();
    assert_eq!(guard.accepted_speed_scaled(100, 2, 0), None);
    assert_eq!(guard.event_stale_limit(), 1000);
    assert_eq!(guard.accepted_speed_scaled(200, 3, 600), None);
    assert_eq!(guard.event_stale_limit(), 900);
    assert_eq!(guard.poll(200, true, false), None);
    assert_eq!(guard.poll(300, true, false), None);
    assert_eq!(guard.accepted_speed_scaled(400, 4, 200), None);
    assert_eq!(guard.event_stale_limit(), 300);
    assert_eq!(guard.poll(500, true, false), None);
    assert_eq!(guard.poll(600, true, false), None);
    assert_eq!(guard.accepted_speed_scaled(650, 5, 900), None);
    assert_eq!(guard.event_stale_limit(), 300);
    assert_eq!(guard.poll(750, true, false), None);
    assert_eq!(guard.poll(850, true, false), None);
    assert_eq!(guard.poll(950, true, false), None);
    assert_eq!(guard.poll(951, true, false), Some(Fault::Tracking));
}
