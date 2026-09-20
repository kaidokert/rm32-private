"""Strict final-epoch QE85 decoder, not lock or physical-edge timing proof."""
import re
from drv_qualification_window import words

HEADER = re.compile(r'QUALEVENT epoch=(\d+) n=(\d+) total=(\d+) omitted=(\d+) invalid=0 frozen=1 final_accepted=(\d+) dispatched_only=1 wire=qe85-v1')


def decode(text, required=False, expected_epoch=None, expected_accepted=None):
    lines=[line for line in text.replace('\r','').splitlines()
           if line.startswith(('QUALEVENT','QE85'))]
    if not lines and not required: return None
    match=HEADER.fullmatch(lines[0]) if lines else None
    if not match: raise ValueError('qualification event header missing/invalid')
    epoch,n,total,omitted,final=map(int,match.groups())
    if (not 1<=epoch<=0xffffffff or not 0<=n<=16 or not 0<=total<=0xffffffff
            or not 0<=final<=0xffffffff or total!=n+omitted
            or (omitted and n!=16) or len(lines)!=n+2):
        raise ValueError('qualification event header/count bounds')
    if expected_epoch is not None and epoch!=expected_epoch:
        raise ValueError('qualification event wrong epoch')
    if expected_accepted is not None and final!=expected_accepted:
        raise ValueError('qualification event wrong final accepted count')
    rows=[]
    for index,line in enumerate(lines[1:]):
        if not line.startswith('QE85 '): raise ValueError('qualification event framing')
        w=words(line[5:],13)
        if w[0]|w[1]<<16!=epoch: raise ValueError('qualification event row epoch')
        before=w[2]|w[3]<<16
        counts=list(w[4:9]); flags=w[11]; step=flags&255
        present=bool(flags&256); stopped=bool(flags&512); partial=w[12]
        if flags&~0x3ff or partial!=int(index==n):
            raise ValueError('qualification event row flags')
        empty=not any(counts)
        if (not 0<=step<=6 or (step==0)!=empty or
                present!=(counts[2]+counts[3]>0) or
                (not present and (w[9] or w[10])) or
                counts[3]>1 or counts[4]>1 or counts[3]+counts[4]>1 or
                stopped!=(counts[4]==1)):
            raise ValueError('qualification event row semantics')
        if not partial and (empty or counts[3]+counts[4]!=1):
            raise ValueError('qualification event incomplete published row')
        rows.append(dict(accepted_before=before,step=step,counts=counts,
                         first_open=w[9] if present else None,
                         last_open=w[10] if present else None,stopped=stopped,
                         partial=bool(partial)))
    complete,partial=rows[:-1],rows[-1]
    for index,row in enumerate(complete):
        if row['accepted_before']!=omitted+index or (row['stopped'] and index!=n-1):
            raise ValueError('qualification event identity/stop order')
        if index and row['step']!=complete[index-1]['step']%6+1:
            raise ValueError('qualification event sector order')
    stopped=bool(complete and complete[-1]['stopped'])
    if stopped:
        expected=dict(complete[-1],partial=True)
        if partial!=expected or final not in (total-1,total):
            raise ValueError('qualification event stopped partial/final count')
    else:
        if (final!=total or partial['accepted_before']!=total or
                partial['counts'][3:]!=[0,0] or
                (partial['step'] and complete and partial['step']!=complete[-1]['step']%6+1)):
            raise ValueError('qualification event partial/final count')
    return dict(epoch=epoch,rows=complete,partial=partial,omitted=omitted,
                final_accepted=final,stopped_callback=stopped,
                epoch_checked=expected_epoch is not None,
                accepted_checked=expected_accepted is not None,
                dispatched_only=True,physical_edge_time=False,
                stopped_reference_persistence_known=False)
