Raw mapping: `27298 − 27277 = 21`; using the supplied post raw/ext pair, the latest accepted entry maps to `4718655 − 21 = 4718634 µs`. That is 11 µs before `at_us=4718645`, and 1018 µs after `last_us=4717616`.

Yes, `from_event=1` plus poll-before-update explains the timestamps without inconsistent freshness. `guard_event` samples `guard_now()` independently of the detector entry. `event()` checks staleness before assigning `last`; `1029 > 1000` therefore latches Stale and returns, preserving the previous watch timestamp. Exactly 1000 µs would pass; this decision is 29 µs beyond the limit.

The post detector record can consequently contain a newer acceptance while the watch retains its previous successful timestamp.

Unsupported inference to remove: 1018 µs is **not a reconstructed detector-to-detector interval**. Its earlier endpoint is a watch timestamp. Neither physical crossing times nor bridge-disable timing are established here.
