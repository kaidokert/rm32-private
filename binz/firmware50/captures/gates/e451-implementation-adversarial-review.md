No concrete cache or shutdown regression is demonstrated by the supplied raw material.

**Concrete blockers / verification gaps:** Cache equivalence requires installation while inactive and every estimator mutation to refresh the cache. Both acceptance paths refresh even after refused/expired arms; COMP cannot re-enter itself, and COM does not consume this cache. However, `det_install` writes cache and estimator separately without enforcing inactivity. Verify its callers establish that prerequisite; otherwise installation can expose mismatched state.

The 393 passing tests support logical equivalence, not hardware integration. The exhaustive snapshot test is essentially an identity; “reseeds” creates fresh objects rather than reinstalling over an existing cache. Source-string assertions do not exercise stop/reinstall interleavings or refused-arm refreshes.

**Assembly / timing not yet measured:** Mapping is removed from pre-arm execution; refresh completes after guard processing (`0x08000bea`). Some arithmetic is shared before the guard, still after arming. Stack allocation grows 52→68 bytes. Persistence instruction count alone does not preserve sampling times: literal reloads and branch behavior changed. Identical scripted reads cannot establish identical live-comparator acceptance.

Post-arm work can delay peer-priority COM and subsequent COMP service; `spent` excludes that delay.

A bounded screen with retained guards is reasonable once inactive installation and stack headroom are established; this is not powered qualification.
