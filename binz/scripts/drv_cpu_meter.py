"""Decode optional software-boundary time partition; not CPU utilization."""
import re
from drv_driven_run import records


def decode_roots(text,cpu):
    headers=re.findall(r'^CPUROOT ([^\r\n]+)',text,re.M)
    rows=records(text,'CR85',3)
    if not headers and not rows:return None
    if (headers!=['contexts=6 includes_nested=1 exclusive=0 extra_clock_reads=0']
            or cpu['kind']!='irq_union' or not cpu['valid']
            or len(rows)!=6 or sorted(r[0] for r in rows)!=list(range(1,7))):
        raise ValueError('invalid root IRQ attribution protocol')
    rows.sort()
    times=[r[1]|r[2]<<16 for r in rows]
    if sum(times)!=cpu['partition_us'][1]:
        raise ValueError('root attribution does not partition IRQ union')
    return dict(includes_nested=True,exclusive=False,extra_clock_reads=0,
                contexts=list(range(1,7)),partition_us=times)


def decode(text):
    union = bool(re.search(r'^(?:CPUUNION|CU85) ', text, re.M))
    if union and re.search(r'^(?:CPUMETER|CPU85) ', text, re.M):
        raise ValueError('mixed CPU accounting protocols')
    count = 2 if union else 7
    lines = re.findall(r'^' + ('CPUUNION' if union else 'CPUMETER') + r' ([^\r\n]+)', text, re.M)
    pattern = (r'elapsed_us=(\d+) started=([01]) active=([01]) fault=([0-4]) '
               r'max_gap_us=(\d+) max_depth=([0-6]) stopped_depth=([0-6]) '
               rf'contexts={count} first_guard_origin=1 exception_overhead_separate=0 '
               r'probe_cost_measured=0 foreground_is_idle=0')
    match = re.fullmatch(pattern, lines[0]) if len(lines) == 1 else None
    if not match:
        raise ValueError('missing/duplicate/unknown CPU meter header')
    elapsed, started, active, fault, gap, depth, stopped = map(int, match.groups())
    rows = records(text, 'CU85' if union else 'CPU85', 5)
    if len(rows) != count or sorted(r[0] for r in rows) != list(range(count)):
        raise ValueError('missing/duplicate CPU contexts')
    rows.sort()
    calls = [r[1] | r[2] << 16 for r in rows]
    times = [r[3] | r[4] << 16 for r in rows]
    if (active or elapsed > 600_000_000 or sum(times) != elapsed or calls[0]
            or stopped > depth or gap > 65535
            or (not started and (elapsed or fault or gap or depth or stopped or any(calls)))
            or (not fault and gap > 1000)
            or any(times[i] and not calls[i] for i in range(1, count))):
        raise ValueError('inconsistent CPU accounting')
    return dict(kind='irq_union' if union else 'per_vector', elapsed_us=elapsed, calls=calls, partition_us=times,
                valid=bool(started and elapsed and not fault), fault=fault,
                cpu_utilization_percent=None, probe_cost_measured=False,
                foreground_is_idle=False, first_guard_origin=True)
