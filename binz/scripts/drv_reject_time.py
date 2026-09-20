"""Late-mismatch callback coordinates, not physical-edge timestamps."""
import re
from drv_qualification_window import words
from drv_qualification_direct import decode as direct_decode,decode_campaign


def decode(text,required=False,*,campaign=True):
    lines=[s for s in text.replace('\r','').splitlines() if s.startswith(('REJECTTIME','RT85'))]
    if not lines and not required:return None
    m=re.fullmatch(r'REJECTTIME epoch=(\d+) n=(\d+) total=(\d+) omitted=(\d+) final_accepted=(\d+) invalid=0 callback_coordinates=1 wire=rt85-v1',lines[0]) if lines else None
    if not m:raise ValueError('late rejection header invalid')
    epoch,n,total,omitted,final=map(int,m.groups())
    if not 0<=total<0xffffffff or n!=min(total,8) or omitted!=total-n or len(lines)!=n+1:
        raise ValueError('late rejection count bounds')
    direct=(decode_campaign(text,True) if campaign else direct_decode(text,True))
    if direct['wire_version']!=2 or epoch!=direct['epoch'] or final!=direct['final_accepted']:
        raise ValueError('late rejection direct binding mismatch')
    buckets={r['accepted_before']:r for r in direct['rows']+[direct['partial']]}
    rows=[]
    for i,line in enumerate(lines[1:]):
        if not line.startswith('RT85 '):raise ValueError('late rejection frame')
        # 28 payload bytes + 4 CRC bytes = eight complete Ascii85 groups.
        # Python's decoder silently ignores a final one-character group.
        if not re.fullmatch(r'[!-u]{40}',line[5:].replace('z','!!!!!')):
            raise ValueError('late rejection encoded length/alphabet')
        el,eh,sl,sh,al,ah,dispatch,index,first,start,pwm,guard,end,reserved=words(line[5:],14)
        accepted=al|(ah<<16)
        if el|(eh<<16)!=epoch or sl|(sh<<16)!=omitted+i or reserved:
            raise ValueError('late rejection epoch/sequence/reserved')
        if accepted>final or not dispatch or not first or not 1<=index<=11:
            raise ValueError('late rejection visit bounds')
        if rows and (accepted,dispatch)<=(rows[-1]['accepted_before'],rows[-1]['dispatch']):
            raise ValueError('late rejection visit order')
        bucket=buckets.get(accepted)
        if bucket and (dispatch>bucket['dispatched'] or index not in bucket['rejected_read_indices']):
            raise ValueError('late rejection disagrees with direct bucket')
        rows.append(dict(sequence=omitted+i,accepted_before=accepted,dispatch=dispatch,
                         rejected_read_index=index,first_read_ticks=first,
                         callback_interval_ticks=start,pwm_count=pwm,guard_count=guard,
                         bracket_end_ticks=end,bracket_delta_mod65536=(end-start)&65535))
    return dict(epoch=epoch,rows=rows,total=total,omitted=omitted,
                physical_edge_time=False,preemption_proven=False,simultaneous_reads=False,
                interval_tick_us=0.5,wrap_count_known=False)
