One blocker remains in the audit’s fail-closed behavior: an unannotated numeric **tail branch** can still certify clean.

For example:

```text
080000f8 <ADC_COMP>:
 80000f8: e000 b.n 80000fc
080000fc <__aeabi_uidiv>:
 80000fc: 4770 bx lr
```

`prune_unreachable()` stops fallthrough at `b`, but records no call because `target_symbol()` returns `None`. Consequently, `reachable()` contains only `ADC_COMP`, with no unresolved edge, indirect flag, or loop. The forbidden helper is missed.

Resolve numeric branch destinations across functions, or fail closed when a reachable branch destination cannot be accounted for. Add this as an eighth fixture.

The three stated call corrections are present in the supplied source. This remaining hole does **not** establish that the supplied ELF exercises it; the four-root PASS log cannot settle that without raw disassembly.
