**Withhold admission on the exact source shown: `request_errors()` has no `return fails`.** Every request returns `None`, including forbidden overrides and mismatched duties. A caller checking truthiness would silently accept these. If the excerpt is truncated, provide the complete function and admission call site; the reported passing tests do not resolve this discrepancy.

From raw evidence:

- Target hold: **20.278 s**, below 30 s qualification.
- Tail: `30163 × 10⁶ / (2277929 × 6)` = **2206.9023 eHz**.
- Whole hold: `266314 / (20.278 × 6)` = **2188.8582 eHz**. Different windows explain the difference.
- Sector counts sum to **287757**; adding **1440** coalesced accepts reconciles **289197** accepted events.
- Deadline reason 2, zero forced events, no reported sag trip, and pre/post outputs disabled support one completed 15% exploration.
- Current remains a proxy; neither efficiency nor negative-current behavior is established.

The visible command mapping correctly specifies `8 → 250`, forbidding pre-keys and overrides **if errors are actually returned and enforced**. It does not establish the remaining admission gates or minimum-hold enforcement.

Resolve that concrete host defect/evidence gap before admitting 25%. Cooling does not resolve it; these excerpts alone establish no additional firmware blocker.
