"""Exact counter dwell for specified reset-to-reset intervals, in timer ticks.

An explicit model, not an inference of missing hardware reset timestamps.
Each interval starts at counter zero; timer runs continuously until the next
reset/end. Pauses, unknown initial phase and reset-write latency are excluded.
"""


def dwell(intervals, *, period=6400, bins=32):
    if period<=0 or bins<=0 or period%bins:
        raise ValueError('period must divide into equal integer bins')
    width=period//bins
    counts=[0]*bins
    for ticks in intervals:
        if not isinstance(ticks,int) or ticks<0:raise ValueError('invalid interval')
        whole,tail=divmod(ticks,period)
        for i in range(bins):
            counts[i]+=whole*width+max(0,min(width,tail-i*width))
    return counts
