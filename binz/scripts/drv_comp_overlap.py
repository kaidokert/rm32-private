"""Stop-context overlap only, not delay attribution or persistence history."""
import re
from drv_cpu_meter import decode as cpu_decode


def decode(text,required=False):
    rows=re.findall(r'^COMPOVERLAP(?: ([^\r\n]*))?$',text,re.M)
    if not rows and not required:return None
    match=re.fullmatch(r'stop_in_comp=([01]) mask=(0|2|64|66) guard_bit=2 dma_bit=64 extra_clock_reads=0 stop_context_only=1 preentry_delay_measured=0',rows[0]) if len(rows)==1 else None
    if not match:raise ValueError('invalid overlap provenance')
    present,mask=map(int,match.groups());cpu=cpu_decode(text)
    if not cpu['valid'] or cpu['kind']!='irq_union' or (not present and mask):
        raise ValueError('invalid overlap meter or context')
    return dict(stop_in_comp=bool(present),guard_overlap=bool(mask&2),
                dma_overlap=bool(mask&64),stop_context_only=True,
                preentry_delay_measured=False,cause_identified=False)
