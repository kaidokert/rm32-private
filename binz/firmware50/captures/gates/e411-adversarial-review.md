The narrow outcome is defensible, with these corrections:

- **50%:** one deadline completion with **19.778 s reported target hold**, not a 30 s qualification. Arithmetic agrees: 298813/19.778 ≈15108 accepted events/s; dividing by six gives ≈2518 eHz. The reported 2525 eHz derives from rounded 66 µs intervals; coast 2524 eHz supports approximate speed agreement.
- **60% request:** sag stop with **zero target hold**. Terminal CCR746/1333 ≈**55.964%**, supporting “stopped around 56% during the ramp,” not “60% failed.” Commanded duty/ceiling600 does not establish applied60%. Zero-hold rate166666 eHz and current0 are invalid summary artifacts.
- **Counter coverage:** all six 50% sector counters and two phase bins saturate at65535; they cannot establish balance or distribution. Tail span3.778 s exceeds its labeled2 s window and needs explanation. Zero margin counters provide no timing coverage; late_arms=0 cannot prove physical deadlines. POSTblend ci_min does not bound the preblend wait estimate.
- **Protection:** the 50% capture records raw950 run3 without a sag trip. Audit whether those statistics and the guard use identical normalization, thresholds, and eligibility before claiming guard consistency. Rest-bus recovery cannot exclude transient supply effects.

Suggested conclusion: “This candidate completed one 50% hold and sag-stopped around56%; changing the wait dependency did not eliminate failure in this screen.” No causal isolation or current/thermal/peak certification follows.

Proceed only with the offline bridge intermediate-write audit.
