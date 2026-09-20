//! Real pure startup policies; deliberately do not link the MCU runtime.
#[path="../examples/support/detector.rs"] mod detector;
#[path="../examples/support/driven_observer.rs"] mod driven_observer;
#[path="../examples/support/average_current.rs"] mod average_current;
#[path="../examples/support/nominal_current.rs"] mod nominal_current;
#[path="../examples/support/startup_feedback.rs"] mod startup_feedback;
#[path="../examples/support/sampled_clock.rs"] mod sampled_clock;
#[path="../examples/support/duty_split.rs"] mod duty_split;
#[path="../examples/support/sixstep.rs"] mod sixstep;
