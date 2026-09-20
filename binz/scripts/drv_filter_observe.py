"""Validate observer-only aggregates; never equates capture availability with qzc."""
import re

def verify(text,expected_code=None,require_detail=False,require_raw=False):
    configs=re.findall(r'^FILTERCONFIG .*$',text.replace('\r',''),re.M)
    if configs or expected_code is not None:
        if len(configs)!=1:raise ValueError('filter config count')
        m=re.fullmatch(r'FILTERCONFIG code=(12|15) ckd=2 observer_only=1',configs[0])
        if not m or expected_code is not None and int(m[1])!=expected_code:
            raise ValueError('filter config mismatch')
    lines=re.findall(r'^FILTEROBS .*$',text.replace('\r',''),re.M)
    if len(lines)!=1:raise ValueError('filter observer row count')
    m=re.fullmatch(r'FILTEROBS samples=(\d+) captures=(\d+) overcapture=(\d+) lag_min_ticks=(\d+) lag_max_ticks=(\d+) active=0 authority=0 latest_capture=1 settling_excluded=0',lines[0])
    if not m:raise ValueError('filter observer format/authority/stopped')
    n,c,o,lo,hi=map(int,m.groups())
    if not 0<=c<=n or not 0<=o<=c:raise ValueError('filter observer counts')
    if c:
        if not 0<=lo<=hi<=65535:raise ValueError('filter observer lag')
    elif (lo,hi)!=(4294967295,0):raise ValueError('filter observer empty extrema')
    result=dict(samples=n,captures=c,overcapture=o,lag_min_ticks=lo,lag_max_ticks=hi)
    sectors=re.findall(r'^FILTERSECTOR .*$',text.replace('\r',''),re.M)
    if sectors or require_detail:
        if len(sectors)!=6:raise ValueError('filter sector count')
        values=[]
        for step,line in enumerate(sectors,1):
            m=re.fullmatch(r'FILTERSECTOR step=(\d+) samples=(\d+) captures=(\d+)',line)
            if not m:raise ValueError('filter sector format')
            i,s,k=map(int,m.groups())
            if i!=step or k>s:raise ValueError('filter sector state')
            values.append((s,k))
        if (sum(v[0] for v in values),sum(v[1] for v in values))!=(n,c):
            raise ValueError('filter sector totals')
        misses=re.findall(r'^FILTERMISS .*$',text.replace('\r',''),re.M)
        if len(misses)!=min(n-c,8):raise ValueError('filter miss prefix count')
        parsed=[]
        for i,line in enumerate(misses):
            m=re.fullmatch(r'FILTERMISS index=(\d+) step=(\d+) rising=(\d+) raw_after=(\d+) counter=(\d+) arm_age=(\d+) pwm=(\d+)',line)
            if not m:raise ValueError('filter miss format')
            j,step,rising,raw,cnt,age,pwm=map(int,m.groups())
            if j!=i or not 1<=step<=6 or max(rising,raw)>1 or max(cnt,age)>65535 or pwm>=2666:
                raise ValueError('filter miss range')
            parsed.append(dict(step=step,rising=rising,raw_after=raw,counter=cnt,arm_age=age,pwm=pwm))
        result.update(sectors=values,miss_prefix=parsed)
    rawrows=re.findall(r'^FILTERRAW .*$',text.replace('\r',''),re.M)
    if rawrows or require_raw:
        if len(rawrows)!=min(n-c,8):raise ValueError('filter raw prefix count')
        raw=[]
        for index,line in enumerate(rawrows):
            m=re.fullmatch(r'FILTERRAW index=(\d+) ready=(\d+) captured=(\d+) over=(\d+) ccr=(\d+) counter=(\d+) period_us=201 age_modulo_only=1',line)
            if not m:raise ValueError('filter raw format')
            i,ready,cap,over,ccr,cnt=map(int,m.groups())
            if i!=index or max(ready,cap,over)>1 or max(ccr,cnt)>200 or not ready and (cap or over):
                raise ValueError('filter raw state')
            raw.append(dict(ready=ready,captured=cap,over=over,age_modulo_us=(cnt-ccr)%201 if ready and cap else None))
        result['raw_prefix']=raw
    return result
