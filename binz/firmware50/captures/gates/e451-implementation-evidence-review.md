Raw assembly confirms the cache load at `0x0800081e`, timer start at `0x08000a08`, and depth refresh store at `0x08000bea`. Mapping occurs after arming and guard handling. Stack allocation increases from 52 to 68 bytes; persistence instruction spacing changes.

**Concrete blockers:** No demonstrated cache or shutdown defect in the supplied paths. Both accepted paths refresh even when arming fails; refusals preserve the estimate. COMP cannot re-enter itself, and COM does not consume this cache. Refresh after a guard trip does not re-enable anything.

Initialization equivalence depends on installation occurring while the detector is inactive: `det_install` writes the cache before replacing `zc`, without masking that entire sequence. The caller establishing this precondition is absent. Verify that ownership condition before powered admission; if installation can overlap active COMP, fix synchronization first.

The 393 passing tests support logical equivalence, but the exhaustive snapshot test is an identity check. Histories do not exercise hardware reinstall, refused/expired arms, or interrupt interleavings; source-string assertions cannot establish those behaviors.

**Timing not yet measured:** Earlier live sampling and changed loop spacing can change accepted crossings. Post-arm refresh can delay peer-priority COM; diagnostics can further perturb timing. A bounded screen with retained guards is reasonable once installation exclusion and stack headroom are established.
