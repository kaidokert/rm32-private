//! Owned no_std state for actual minz-core clusters; adapted from frozen reference mock.
#![allow(dead_code)]
use minz_core::am32::{ZCT_REC, ZctRing};
use minz_core::am32_loop::{Bench, Drive, Duty, Sched};
use minz_core::zct_trace::ZctTrace;
use portable_atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};
#[derive(Default)]
pub struct SchedStore {
    pub commutation_interval: AtomicU32,
    pub interval_hist: [AtomicU32; 6],
    pub average_interval: AtomicU32,
    pub last_average_interval: AtomicU32,
    pub last_zc: AtomicU16,
    pub this_zc: AtomicU16,
    pub wait_time: AtomicU16,
}
impl SchedStore {
    pub const fn new() -> Self {
        Self {
            commutation_interval: AtomicU32::new(0),
            interval_hist: [const { AtomicU32::new(0) }; 6],
            average_interval: AtomicU32::new(0),
            last_average_interval: AtomicU32::new(0),
            last_zc: AtomicU16::new(0),
            this_zc: AtomicU16::new(0),
            wait_time: AtomicU16::new(0),
        }
    }

    pub const fn sched(&self) -> Sched<'_> {
        Sched {
            commutation_interval: &self.commutation_interval,
            interval_hist: &self.interval_hist,
            average_interval: &self.average_interval,
            last_average_interval: &self.last_average_interval,
            last_zc: &self.last_zc,
            this_zc: &self.this_zc,
            wait_time: &self.wait_time,
        }
    }
}

#[derive(Default)]
pub struct DriveStore {
    pub current_step: AtomicU16,
    pub rising: AtomicBool,
    pub old_routine: AtomicBool,
    pub running: AtomicBool,
    pub zcfound: AtomicBool,
    pub bemf_counter: AtomicU16,
    pub min_bemf_up: AtomicU16,
    pub min_bemf_down: AtomicU16,
    pub zero_crosses: AtomicU32,
    pub filter_level: AtomicU16,
    pub bad_count: AtomicU16,
    pub desync_check: AtomicBool,
    pub desync_happened: AtomicU32,
    pub bemf_timeout_happened: AtomicU32,
    pub tenkhz_counter: AtomicU16,
    pub zcfr_guard_hits: AtomicU32,
}
impl DriveStore {
    pub const fn new() -> Self {
        Self {
            current_step: AtomicU16::new(0),
            rising: AtomicBool::new(false),
            old_routine: AtomicBool::new(false),
            running: AtomicBool::new(false),
            zcfound: AtomicBool::new(false),
            bemf_counter: AtomicU16::new(0),
            min_bemf_up: AtomicU16::new(0),
            min_bemf_down: AtomicU16::new(0),
            zero_crosses: AtomicU32::new(0),
            filter_level: AtomicU16::new(0),
            bad_count: AtomicU16::new(0),
            desync_check: AtomicBool::new(false),
            desync_happened: AtomicU32::new(0),
            bemf_timeout_happened: AtomicU32::new(0),
            tenkhz_counter: AtomicU16::new(0),
            zcfr_guard_hits: AtomicU32::new(0),
        }
    }

    pub const fn drive(&self) -> Drive<'_> {
        Drive {
            current_step: &self.current_step,
            rising: &self.rising,
            old_routine: &self.old_routine,
            running: &self.running,
            zcfound: &self.zcfound,
            bemf_counter: &self.bemf_counter,
            min_bemf_up: &self.min_bemf_up,
            min_bemf_down: &self.min_bemf_down,
            zero_crosses: &self.zero_crosses,
            filter_level: &self.filter_level,
            bad_count: &self.bad_count,
            desync_check: &self.desync_check,
            desync_happened: &self.desync_happened,
            bemf_timeout_happened: &self.bemf_timeout_happened,
            tenkhz_counter: &self.tenkhz_counter,
            zcfr_guard_hits: &self.zcfr_guard_hits,
        }
    }
}

#[derive(Default)]
pub struct DutyStore {
    pub input: AtomicU16,
    pub adjusted_input: AtomicU16,
    pub uart_duty_input: AtomicU16,
    pub duty_cycle_setpoint: AtomicU16,
    pub duty_cycle: AtomicU16,
    pub last_duty_cycle: AtomicU16,
    pub duty_cycle_maximum: AtomicU16,
    pub ramp_count: AtomicU16,
    pub killed: AtomicBool,
    pub kill_reason: AtomicU16,
    pub tim1_arr: AtomicU16,
}
impl DutyStore {
    pub const fn new() -> Self {
        Self {
            input: AtomicU16::new(0),
            adjusted_input: AtomicU16::new(0),
            uart_duty_input: AtomicU16::new(0),
            duty_cycle_setpoint: AtomicU16::new(0),
            duty_cycle: AtomicU16::new(0),
            last_duty_cycle: AtomicU16::new(0),
            duty_cycle_maximum: AtomicU16::new(0),
            ramp_count: AtomicU16::new(0),
            killed: AtomicBool::new(false),
            kill_reason: AtomicU16::new(0),
            tim1_arr: AtomicU16::new(0),
        }
    }

    pub fn duty(&self) -> Duty<'_> {
        Duty {
            input: &self.input,
            adjusted_input: &self.adjusted_input,
            uart_duty_input: &self.uart_duty_input,
            duty_cycle_setpoint: &self.duty_cycle_setpoint,
            duty_cycle: &self.duty_cycle,
            last_duty_cycle: &self.last_duty_cycle,
            duty_cycle_maximum: &self.duty_cycle_maximum,
            ramp_count: &self.ramp_count,
            killed: &self.killed,
            kill_reason: &self.kill_reason,
            tim1_arr: &self.tim1_arr,
        }
    }
}

#[derive(Default)]
pub struct BenchStore {
    pub uart_deadman_ticks: AtomicU32,
    pub i_raw: AtomicU16,
    pub vbat_raw: AtomicU16,
    pub oc_acc: AtomicU32,
    pub oc_cnt: AtomicU32,
    pub vbat_low_ticks: AtomicU32,
    pub vbat_floor_raw: AtomicU16,
    pub stop_req: AtomicBool,
    pub dump_req: AtomicBool,
    pub info_req: AtomicBool,
    pub gecko_req: AtomicBool,
    pub wax_req: AtomicBool,
    pub freerun_req: AtomicBool,
    pub hist_req: AtomicBool,
    pub zct_stream_on: AtomicBool,
    pub delay_in_free: AtomicU32,
    pub delay_out_free: AtomicU32,
}
impl BenchStore {
    pub const fn new() -> Self {
        Self {
            uart_deadman_ticks: AtomicU32::new(0),
            i_raw: AtomicU16::new(0),
            vbat_raw: AtomicU16::new(0),
            oc_acc: AtomicU32::new(0),
            oc_cnt: AtomicU32::new(0),
            vbat_low_ticks: AtomicU32::new(0),
            vbat_floor_raw: AtomicU16::new(0),
            stop_req: AtomicBool::new(false),
            dump_req: AtomicBool::new(false),
            info_req: AtomicBool::new(false),
            gecko_req: AtomicBool::new(false),
            wax_req: AtomicBool::new(false),
            freerun_req: AtomicBool::new(false),
            hist_req: AtomicBool::new(false),
            zct_stream_on: AtomicBool::new(false),
            delay_in_free: AtomicU32::new(0),
            delay_out_free: AtomicU32::new(0),
        }
    }

    pub fn bench(&self) -> Bench<'_> {
        Bench {
            uart_deadman_ticks: &self.uart_deadman_ticks,
            i_raw: &self.i_raw,
            vbat_raw: &self.vbat_raw,
            oc_acc: &self.oc_acc,
            oc_cnt: &self.oc_cnt,
            vbat_low_ticks: &self.vbat_low_ticks,
            vbat_floor_raw: &self.vbat_floor_raw,
            stop_req: &self.stop_req,
            dump_req: &self.dump_req,
            info_req: &self.info_req,
            gecko_req: &self.gecko_req,
            wax_req: &self.wax_req,
            freerun_req: &self.freerun_req,
            hist_req: &self.hist_req,
            zct_stream_on: &self.zct_stream_on,
            delay_in_free: &self.delay_in_free,
            delay_out_free: &self.delay_out_free,
        }
    }
}

pub const ZCT_CAPACITY: usize = 64;
pub struct ZctStore {
    pub ring: [[AtomicU16; ZCT_REC]; ZCT_CAPACITY],
    pub head: AtomicUsize,
    pub tail: AtomicUsize,
    pub drop: AtomicU32,
    pub comm_n: AtomicU32,
    pub batching: AtomicBool,
}
impl ZctStore {
    pub const fn new() -> Self {
        Self {
            ring: [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_CAPACITY],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            drop: AtomicU32::new(0),
            comm_n: AtomicU32::new(0),
            batching: AtomicBool::new(false),
        }
    }
    pub fn zct(&self) -> ZctTrace<'_, ZCT_CAPACITY> {
        ZctTrace {
            ring: ZctRing {
                ring: &self.ring,
                head: &self.head,
                tail: &self.tail,
                drop: &self.drop,
            },
            comm_n: &self.comm_n,
            batching: &self.batching,
        }
    }
    pub fn records(&self) -> usize {
        (self.head.load(Ordering::Relaxed) + ZCT_CAPACITY - self.tail.load(Ordering::Relaxed))
            % ZCT_CAPACITY
    }
}
