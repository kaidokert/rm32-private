use super::comparator_resume_allowed as permit;

#[test]
fn resume_truth_table() {
    for guard in [0, 8] {
        for detector in [false, true] {
            for driven in [false, true] {
                for stopped in [false, true] {
                    for active in [false, true] {
                        for phase in 0..=4 {
                            let expected = guard == 0
                                && if detector {
                                    !stopped && active && phase == 0
                                } else {
                                    driven
                                };
                            assert_eq!(permit(guard, detector, driven, stopped, active, phase), expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn stop_and_new_blank_override_old_permission() {
    assert!(permit(0, true, true, false, true, 0));
    assert!(!permit(8, false, false, true, false, 0));
    assert!(!permit(0, true, true, false, true, 3));
    assert!(permit(0, false, true, true, false, 0));
    assert!(!permit(0, false, false, false, false, 0));
}
