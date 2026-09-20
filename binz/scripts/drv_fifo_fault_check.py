"""Check a retained FIFO-full injection and subsequent reuse capture pair.

Same-boot provenance comes from the bench log, not UART capture contents alone.
"""
import argparse
from pathlib import Path
from drv_sustained_report import fields,summarize
from drv_current_sums import decode


def verify(stalled,reused):
    a=summarize(stalled);b=summarize(reused)
    checks=[a['outcome']=='powered_stopped',a['powered_reason']=='11',
            fields(stalled,'DMAFAULT').get('code')=='9',
            fields(stalled,'DMAFEEDBACK').get('queue_peak')=='8',
            fields(stalled,'FIFOFAULT').get('state')=='1',decode(stalled)['n']==20000,
            b['outcome']=='powered_window_complete',
            fields(reused,'FIFOFAULT').get('state')=='2',decode(reused)['n']>20000,
            int(a['stack_untouched_bytes'])>=512,int(b['stack_untouched_bytes'])>=512]
    if not all(checks): raise ValueError('FIFO refusal/reuse evidence does not match test contract')
    return {'queue_full_verified':True,'reuse_window_verified':True,
            'same_boot_requires_bench_provenance':True}


if __name__=='__main__':
    import json
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('stalled',type=Path);p.add_argument('reused',type=Path)
    a=p.parse_args();print(json.dumps(verify(a.stalled.read_text(),a.reused.read_text()),indent=2))
