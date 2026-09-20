"""CRC-checked accepted-event time bins. Never qZC coverage or rotor-lock proof."""
import argparse
import base64
import re
import struct
import zlib
from pathlib import Path


def unique_fields(text,prefix):
    lines=re.findall(r'^'+re.escape(prefix)+r' ([^\r\n]+)',text,re.M)
    if len(lines)!=1: raise ValueError('missing/duplicate '+prefix)
    return {k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',lines[0])}


def decode(text,label='ET85'):
    if label not in ('ET85','FT85'): raise ValueError('unknown timeline')
    h=unique_fields(text,'TIMELINE label='+label)
    if not {'bins','window_us','bin_us','events','overrun_events','accepted_only'}<=h.keys():
        raise ValueError('incomplete timeline header')
    if h['bins']!=6 or h['accepted_only']!=1 or not 20000<=h['window_us']<=600000000:
        raise ValueError('timeline envelope')
    if h['bin_us']!=(h['window_us']+5)//6: raise ValueError('bin width')
    if label=='ET85':
        q=unique_fields(text,'ACCEPTQUALITY')
        if unique_fields(text,'TIMELINEREFUSED').get('n')!=0: raise ValueError('refused timeline timestamps')
        total,end=q['events'],q['end_us']
        if q.get('last_us',0)>end: raise ValueError('post-stop event')
    else:
        q=unique_fields(text,'FIRSTSEG')
        total,end=q['events'],q['end_us']
    bins=[]
    for line in text.splitlines():
        if not line.startswith(label+' '): continue
        raw=base64.a85decode(line[len(label)+1:])
        if len(raw)!=14 or zlib.crc32(raw[:10])!=int.from_bytes(raw[10:],'little'):
            raise ValueError('timeline length/CRC')
        index,lo,hi,minimum,maximum=struct.unpack('<5H',raw[:10])
        count=lo+(hi<<16)
        if index!=len(bins) or index>=6: raise ValueError('timeline bin order')
        if (not count and (minimum,maximum)!=(65535,0)) or (count and minimum>maximum and (minimum,maximum)!=(65535,0)):
            raise ValueError('timeline gap range')
        start=index*h['bin_us']
        if count and start>end: raise ValueError('events in unobserved bin')
        stop=min(end,h['window_us'],start+h['bin_us'])
        if index==5 and end>h['window_us']: stop=end
        bins.append(dict(index=index,events=count,min_gap_us=minimum,max_gap_us=maximum,
                         start_us=start,observed_us=max(0,stop-start),
                         saturated=maximum==65535))
    if len(bins)!=6 or sum(b['events'] for b in bins)!=h['events'] or h['events']!=total:
        raise ValueError('timeline event accounting')
    if h['overrun_events']>bins[-1]['events'] or (end<h['window_us'] and h['overrun_events']):
        raise ValueError('timeline overrun accounting')
    return dict(header=h,bins=bins,end_us=end)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path);ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    if args.out.exists(): ap.error('refusing to overwrite plot')
    text=args.capture.read_text()
    from drv_sustained_report import summarize
    summarize(text)  # CRC/event-window/final-off validation
    labels=['FT85','ET85'] if 'TIMELINE label=FT85 ' in text else ['ET85']
    traces=[decode(text,l) for l in labels]
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    fig,axes=plt.subplots(2,len(traces),squeeze=False,figsize=(6*len(traces),6))
    for col,(label,t) in enumerate(zip(labels,traces)):
        bins=t['bins'];x=[b['start_us']/1e6 for b in bins]
        rates=[b['events']*1e6/b['observed_us'] if b['observed_us'] else float('nan') for b in bins]
        axes[0,col].bar(x,rates,width=[b['observed_us']/1e6*.85 for b in bins],align='edge')
        axes[0,col].set(title=label+' (separate segment clock)',ylabel='Accepted events/s')
        for key in ['min_gap_us','max_gap_us']:
            axes[1,col].plot(x,[b[key] if b['events'] and b['max_gap_us'] else float('nan') for b in bins],marker='o',label=key)
        axes[1,col].set(xlabel='Segment time (s)',ylabel='Inter-event gap (us)')
        axes[1,col].legend()
        for ax in axes[:,col]:
            ax.axvline(t['end_us']/1e6,color='red',linestyle=':',label='recorded stop')
        if not t['header']['events']:
            axes[0,col].text(.5,.5,'No accepted events',transform=axes[0,col].transAxes,ha='center')
    fig.suptitle('Accepted-event timing, not independent qZC or rotor-lock proof')
    fig.tight_layout();fig.savefig(args.out);plt.close(fig)


if __name__=='__main__': main()
