Both 15% runs achieved **20.278 s at target**, from 22.778 s closed-loop minus 2.500 s ramp. Applied duty was approximately **14.93%** (199/1333).

- E480 accepted rate: 262138/20.278 = **12,927.21/s**; control: **12,909.51/s**, only **+0.137%**. Tail rates were approximately 12,977 versus 12,955/s using the actual **2.278 s spans**, not the nominal 2 s window.
- The 995‰ depression watch increased substantially: **75,830/266,218 = 28.48%** versus **3,428/268,190 = 1.28%** of drive scans; longest runs 52 versus 20. Different bus references limit interpretation. Current proxies 249/211 mA are uncalibrated.
- Both report zero late arms, thin events and forced commutations; preparation maxima 11/12 µs, minimum CI 47/47 µs, commutation lateness 13/13 µs. Both stopped with reason=2, guard=0 and safe POSTSTOP. Zero margin histograms establish no measured margin.

**Supported:** comparable sustained propless operation at 15%, without demonstrated superiority or fault-path validation.

The prior 25% LateArm occurred after only **11 ms target exposure**; the 15% dwell is approximately **1,843 times longer at a different operating point**. The proposed same-image 25% screen meaningfully probes that failing rung, but cannot isolate the narrower commit’s effect or establish qualification.
