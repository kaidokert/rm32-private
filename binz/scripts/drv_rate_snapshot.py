"""Terminal state only: never infer the history of preceding IRQ dispatches."""
from drv_driven_run import records

HEADER = 'RATEGATE fields=count,average,rpr18,fpr18,hal_level,rising,epoch,bracket_us terminal_only=1 sequential_reads=1'

def decode(text):
    if text.count(HEADER) != 1:
        raise ValueError('missing/surplus rate snapshot provenance')
    rows = records(text, 'RG85', 8)
    if not rows:
        return None
    if len(rows) != 1:
        raise ValueError('surplus rate snapshot')
    count, average, rp, fp, level, rising, epoch, bracket = rows[0]
    if average != 1666 or any(v not in (0, 1) for v in (rp, fp, level, rising)):
        raise ValueError('invalid fixed-observer snapshot')
    return dict(count=count, gate=average >> 1, gate_open=count > (average >> 1),
                pending=bool(rp or fp), post_zc=level == rising,
                epoch=epoch, bracket_us=bracket, historical_cause_proven=False)
