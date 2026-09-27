[P1] Unannotated direct **tail branches** still bypass the corrected unresolved-call handling. For example:

```text
080000f8 <ADC_COMP>:
 80000f8: e002 b.n 8000100
08000100 <__aeabi_uidiv>:
 8000100: 4770 bx lr
```

`prune_unreachable()` retains the branch but records neither a call nor an unresolved transfer because `target_symbol()` returns `None`. Consequently, `reachable()` contains only `ADC_COMP`, and the audit certifies it despite the reachable forbidden helper.

The unannotated `bl` correction therefore leaves an equivalent tail-call hole. Resolve numeric branch destinations against instruction/symbol addresses, or fail closed when a reachable branch destination cannot be established as an internal edge. Add this fixture with an expected audit failure.

The seven passing fixtures and same-ELF result do not cover this case.
