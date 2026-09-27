The material corrections hold by inspection: unannotated external tail branches become unresolved, numeric internal targets remain traversable, `bx lr` is a return, and other `bx` operands fail as indirect transfers. The supplied captures report eight passing tests and four passing disabled-probe roots; I have not independently rerun them.

Concrete remaining defect: reachable `mov pc, r3` is treated as an ordinary instruction, leaving `indirect=False`. A root containing that transfer can be certified despite an unresolved destination. This is a fail-closed gap, not merely a coverage limitation.

The unchanged disabled-probe pass supports that artifact only; it does not establish enabled-path coverage or authorize motors.
