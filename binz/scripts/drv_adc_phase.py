"""Trigger-phase histogram validation; not aperture or sector coverage proof."""
import re
from drv_driven_run import records


def decode(text):
    heads=re.findall(r'^ADCPHASE ([^\r\n]+)',text,re.M)
    rows=records(text,'AP85',3)
    if not heads and not rows:return None
    match=re.fullmatch(r'count=(\d+) bins=32 period_ticks=(6400|3200|2666) trigger_only=1 aperture_known=0 sector_known=0 latency_qualified=0 stopped=1',heads[0]) if len(heads)==1 else None
    if not match or len(rows)!=32 or [r[0] for r in rows]!=list(range(32)):
        raise ValueError('invalid ADC phase histogram metadata/rows')
    bins=[r[1]+(r[2]<<16) for r in rows];count=int(match[1])
    period=int(match[2])
    if period in (2666,3200):
        from drv_role_check import verify_mode
        verify_mode(text,required=True,carrier_hz={1600:40000,2000:32000,2666:24006,3200:20000}[period])
    elif count and re.search(r'^PWMROLES .*carrier_hz=(40000|32000|24006|20000)$',text,re.M):
        raise ValueError('powered capture contains startup-period phase data')
    if count>6_000_000 or sum(bins)!=count:raise ValueError('ADC phase count mismatch')
    return dict(count=count,bins=bins,period_ticks=period,trigger_only=True,latency_qualified=False,
                aperture_known=False,sector_known=False,unbiased_current_proven=False,
                sampling_bias_proven=False,time_occupancy_known=False)
