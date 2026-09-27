Raw evidence: 3/3 host tests passed; serial reports disabled preflight, latch-pad and deadline self-tests PASS. Inherited startup CCRs peak at396/1333 (29.708%); subsequent plans cap at1066. The incremental-cache warning does not negate the test results.

No concrete unresolved defect preventing the proposed bounded 50%/45s protected screen is demonstrated here. The inheritance bound closes the specific startup-over-cap concern.

Limits: the model assumes atomic update behavior; `ccpc` has no modeled effect, and stop positions1–6 collapse to the same post-transaction stop. Thus these tests do not independently prove hardware role/compare interaction or pending-IRQ timing. The reported pad PASS lacks transition traces here.

Proceeding is supportable as an exploratory screen under the stated protections, not qualification. Review actual guard gaps before higher duty; do not automatically extend to60s.
