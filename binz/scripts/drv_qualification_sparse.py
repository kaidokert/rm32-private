"""Strict sparse substream decoding; not outputs-off, lock, or campaign proof."""
import re
from drv_qualification_window import words

HEADER=re.compile(r'QUALSPARSE epoch=(\d+) n=(\d+) omitted=(\d+) invalid=0 frozen=1 capture=([01]) threshold_us=40 dispatch_body=1 edge_time=0 normal_calls_counted=0')

def decode_campaign(text,required=False):
    """Bind only the final observation, not an archived first segment or lock proof."""
    bindings=[line for line in text.replace('\r','').splitlines() if line.startswith('SPARSEBIND')]
    if not bindings and not required:
        return decode(text)
    if len(bindings)!=1:raise ValueError('sparse campaign binding missing/duplicate')
    m=re.fullmatch(r'SPARSEBIND epoch=(\d+) accepted=(\d+) final_observation=1 ram=1',bindings[0])
    if not m:raise ValueError('sparse campaign binding invalid or not RAM probe')
    epoch,accepted=map(int,m.groups())
    if not 1<=epoch<=0xffffffff or not 0<=accepted<=0xffffffff:
        raise ValueError('sparse campaign binding bounds')
    result=decode(text,required=True,expected_epoch=epoch)
    if any(row['accepted_before']+row['accepted']>accepted for row in result['rows']):
        raise ValueError('sparse row beyond final accepted count')
    result.update(final_observation_only=True,final_accepted=accepted,ram_probe=True)
    return result

def decode(text,required=False,expected_epoch=None):
    # Count malformed protocol lines as errors, not as absent evidence.
    lines=[line for line in text.replace('\r','').splitlines()
           if line.startswith(('QUALSPARSE','QS85'))]
    if not lines and not required:return None
    m=HEADER.fullmatch(lines[0]) if lines else None
    if not m:raise ValueError('sparse header missing/invalid')
    epoch,n,omitted,capture=map(int,m.groups())
    if not 1<=epoch<=0xffffffff or not 0<=n<=16 or omitted+n>0xffffffff or (omitted and n!=16):
        raise ValueError('sparse header bounds')
    if expected_epoch is not None and epoch!=expected_epoch:
        raise ValueError('sparse wrong campaign epoch')
    if not capture:
        if required or len(lines)!=1:raise ValueError('sparse capture absent')
        return dict(captured=False,epoch=epoch,rows=None)
    if len(lines)!=n+1:raise ValueError('sparse record count')
    rows=[]
    minimum=0
    for i,line in enumerate(lines[1:]):
        if not line.startswith('QS85 '):raise ValueError('sparse record framing')
        elo,ehi,lo,hi,tick,duration,step,accepted,overlap,stopped=words(line[5:],10)
        before=lo|(hi<<16)
        if elo|(ehi<<16)!=epoch:raise ValueError('sparse row epoch mismatch')
        if not 1<=step<=6 or accepted>1 or overlap>1 or stopped>1:
            raise ValueError('sparse flags')
        if duration<=40 and not stopped:raise ValueError('sparse unselected row')
        if before<minimum or before+accepted>0xffffffff:
            raise ValueError('sparse accepted identity reversal/overflow')
        if stopped and i!=n-1:raise ValueError('sparse rows after stop')
        minimum=before+accepted
        rows.append(dict(accepted_before=before,entry_tick=tick,duration_us=duration,
                         step=step,accepted=bool(accepted),guard_overlap=bool(overlap),stopped=bool(stopped)))
    return dict(captured=True,epoch=epoch,rows=rows,omitted_selected=omitted,
                threshold_us=40,normal_calls_counted=False,physical_edge_time=False,
                preentry_latency_measured=False,entry_tick_unwrapped=False,
                campaign_epoch_checked=expected_epoch is not None)
