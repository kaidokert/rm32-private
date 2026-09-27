Raw checks: ordinal 289561 gives `52485−52458−19 = 8 µs` deadline error. The supplied fine brackets are 2 ticks (0.25 µs), but bracket the post-apply clock read, not the bridge transition. Acceptance endpoints span 9,597 µs over 127 intervals: approximately 2,206 eHz.

Concrete anomaly: `BEMFTAIL span_us=2277943` exceeds its declared 2,000,000 µs window by 13.9%. Its endpoints confirm that span; “last two seconds” would be inaccurate. Sector accepts sum to 287,894, falling 1,767 below `accepted=289661`, exactly matching mailbox coalescing. Those sector counters are therefore not a complete acceptance census.

The bounded same-image 25% screen can discriminate **recorded acceptance cessation versus an accepted ordinal lacking a commutation record**, provided both tails freeze coherently at failure and stop handling preserves pending-event evidence. This successful tail alone does not establish those conditions. A terminal unmatched acceptance can also mean shutdown cancelled its pending COM; distinguish that using its deadline and stop timestamp.

It cannot establish physical crossing validity or physical COM timing. Keep any conclusion specific to this instrumented image; the current proxy and this single run do not support qualification.
