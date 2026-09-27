The material corrections hold by inspection: unannotated external branches fail unresolved; internal destinations remain traversable; `bx lr` terminates traversal; other `bx` operands fail indirect; interior calls remain unresolved. The eight fixtures cover these cases. The capture reports four passing disabled-probe roots; I haven’t independently executed them.

Remaining concrete false negative: Thumb `mov pc, r3` is an indirect control transfer, but the parser treats it as ordinary fallthrough. A function containing that instruction followed by `bx lr` can certify clean despite an unknown destination. This is a fail-closed defect, not merely limited test coverage.

The disabled-suite acceptance remains conditional on closing that gap. No motor authorization is implied.
