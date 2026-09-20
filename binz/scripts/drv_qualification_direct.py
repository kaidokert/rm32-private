"""Strict QD85 final-epoch substream; not persistence or physical-edge proof."""
import re
from drv_qualification_window import words

HEADER=re.compile(r'QUALDIRECT epoch=(\d+) n=(\d+) total=(\d+) omitted=(\d+) invalid=0 active=0 final_accepted=(\d+) dispatched_only=1 partial_until_unwind=1 wire=qd85-v([12])')


def decode_campaign(text,required=False):
    """Final observation binding only; whole-run safety/lock checked separately."""
    lines=text.replace('\r','').splitlines()
    present=any(line.startswith(('DIRECTBIND','QUALDIRECT','QD85')) for line in lines)
    if not present and not required:return None
    bindings=[line for line in lines if line.startswith('DIRECTBIND')]
    if len(bindings)!=1:raise ValueError('direct campaign binding missing/duplicate')
    m=re.fullmatch(r'DIRECTBIND epoch=(\d+) accepted=(\d+) final_observation=1 ram=1',bindings[0])
    if not m:raise ValueError('direct campaign binding invalid or not RAM probe')
    epoch,accepted=map(int,m.groups())
    quality=[line for line in lines if line.startswith('ACCEPTQUALITY')]
    if len(quality)!=1:raise ValueError('direct final accepted summary missing/duplicate')
    q=re.fullmatch(r'ACCEPTQUALITY events=(\d+) first_us=\d+ last_us=\d+ order_bad=\d+ end_us=\d+ lock_proven=0',quality[0])
    if not q or int(q[1])!=accepted:raise ValueError('direct final accepted summary mismatch')
    result=decode(text,True,expected_epoch=epoch,expected_accepted=accepted)
    result.update(final_observation_only=True,ram_probe=True,campaign_lock_proven=False)
    return result


def decode(text,required=False,expected_epoch=None,expected_accepted=None):
    lines=[line for line in text.replace('\r','').splitlines()
           if line.startswith(('QUALDIRECT','QD85'))]
    if not lines and not required:return None
    m=HEADER.fullmatch(lines[0]) if lines else None
    if not m:raise ValueError('direct qualification header missing/invalid')
    epoch,n,total,omitted,final,version=map(int,m.groups())
    if (not 1<=epoch<0xffffffff or not 0<=total<=0xffffffff or
            n!=min(total,16) or omitted!=total-n or final!=total or len(lines)!=n+2):
        raise ValueError('direct qualification count bounds')
    if expected_epoch is not None and epoch!=expected_epoch:
        raise ValueError('direct qualification wrong epoch')
    if expected_accepted is not None and final!=expected_accepted:
        raise ValueError('direct qualification wrong final accepted')
    rows=[]
    for index,line in enumerate(lines[1:]):
        if not line.startswith('QD85 '):raise ValueError('direct qualification frame')
        elo,ehi,lo,hi,step,dispatched,closed,opened,first,last,partial=words(line[5:],11)
        mask=step>>4 if version==2 else 0
        if version==2:step &= 15
        before=lo|(hi<<16)
        if (elo|(ehi<<16))!=epoch or before!=omitted+index:
            raise ValueError('direct qualification epoch/identity')
        if (partial!=int(index==n) or
                (partial and step!=0) or (not partial and not 1<=step<=6)):
            raise ValueError('direct qualification sector/partial')
        if closed+opened>dispatched or (not partial and not opened):
            raise ValueError('direct qualification inconsistent counters')
        if mask.bit_count()>opened-int(not partial):
            raise ValueError('direct rejection positions exceed eligible open visits')
        if (not opened and (first or last)) or (opened and (not first or not last)):
            raise ValueError('direct qualification open-count presence')
        if index and not partial and step!=rows[-1]['step']%6+1:
            raise ValueError('direct qualification sector gap')
        rows.append(dict(accepted_before=before,step=step,dispatched=dispatched,
                         closed=closed,open=opened,no_first_read=dispatched-closed-opened,
                         first_open=first if opened else None,last_open=last if opened else None,
                         partial=bool(partial),
                         rejected_read_indices=[i for i in range(12) if mask&(1<<i)] if version==2 else None))
    return dict(epoch=epoch,rows=rows[:-1],partial=rows[-1],omitted=omitted,
                final_accepted=final,wire_version=version,epoch_checked=expected_epoch is not None,
                accepted_checked=expected_accepted is not None,
                partial_until_unwind=True,dispatched_only=True,
                physical_edge_time=False,persistence_reject_count_known=False)
