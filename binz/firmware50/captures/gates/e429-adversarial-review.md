The supplied assembly supports constant-folded **fresh-estimate** scheduling: the blended, clamped estimate is stored at `0x08000926`; `0x08000932/946/948` compute `(ci >> 1) - (ci >> 2)`. This matches the integer fixed16 function exactly, including rounding; it is not always `ci >> 2`.

No concrete arithmetic regression is evident. Remaining gaps:

- The assertions bind both schedule constants to 16 and demonstrably reject an incompatible configuration. They do **not** establish that every runtime advance write uses those constants. Equivalence requires checking all setters and handover paths, since the wrapper ignores runtime advance.
- The four “identical” reports lack identified artifact pairs and comparison methodology. They cannot simultaneously establish unchanged candidate COMP code and its claimed reduction. Clarify that these compare ordinary-policy builds, if that is their scope.
- The 379 passing tests do not identify coverage comparing `FreshEstimate` against `ConstantAdvance<FreshEstimate, 16>`, including seed acceptance, clamp boundaries, rounding, and rejection paths.
- The excerpt supports eliminated runtime advance arithmetic, but does not independently establish 813→787 executable instructions, unchanged other roots, or a timing bound.

Exact-artifact provenance and equivalence evidence remain prerequisites for justifying the proposed powered screen. Disabled boot checks alone cannot close those gaps.
