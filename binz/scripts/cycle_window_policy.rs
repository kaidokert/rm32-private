//! Host-only candidate; NOT linked into motor firmware.
//! A two-cycle speed envelope is not an instantaneous speed/lock certificate.
//! Caller must independently poll tracking, electrical and deadline guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal { pub step:u8, pub span_us:u32, pub minimum_us:u32, pub slow:bool }

pub struct Window<const FLOOR:u32> {
    at:[u32;6], previous:[u32;6], seen:u8, paired:u8,
    fault:Option<Refusal>,
}
impl<const FLOOR:u32> Window<FLOOR> {
    pub fn new()->Self {
        assert!((6..=6000).contains(&FLOOR));
        Self {at:[0;6],previous:[0;6],seen:0,paired:0,fault:None}
    }
    /// Only after an independent monitor accepts order and <=1000us gaps.
    /// Monotonic wrapping-u32 microseconds; no unobserved full clock wraps.
    pub fn accepted(&mut self,now:u32,step:u8)->Option<Refusal> {
        if self.fault.is_some() {return self.fault;}
        assert!((1..=6).contains(&step));
        let i=(step-1) as usize; let bit=1<<i;
        if self.seen&bit!=0 {
            let one=now.wrapping_sub(self.at[i]);
            // Validate before adding: both operands are <=6000, sum <=12000.
            if one>6000 {
                self.fault=Some(Refusal {step,span_us:one,minimum_us:FLOOR,slow:true});
                return self.fault;
            }
            let (span,minimum)=if self.paired&bit!=0 {
                (one+self.previous[i],2*FLOOR)
            } else {(one,FLOOR)};
            if span<minimum {
                self.fault=Some(Refusal {step,span_us:span,minimum_us:minimum,slow:false});
                return self.fault;
            }
            self.previous[i]=one; self.paired|=bit;
        }
        self.at[i]=now; self.seen|=bit; None
    }
}
