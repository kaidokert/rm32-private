"""Compare initial-segment raw sums with prestart baseline, never calibrated amps.

Reject recovery: the initial baseline is revoked by its driver wake. Even an
initial-stage epoch match does not establish standstill, drift or ADC accuracy.
"""
import argparse
import json
import re
from pathlib import Path
from drv_driven_handoff import verify
from drv_current_sums import decode as sums
from drv_prestart_baseline import decode as baseline


def report(text):
    if re.search(r'^REENTRY ',text,re.M):raise ValueError('cannot apply initial baseline to recovery')
    motor=verify(text)
    if not motor['powered_handoff_window_verified']:raise ValueError('initial powered segment incomplete')
    b=baseline(text);s=sums(text)
    if not b or b['header']['status']!=2 or b['header']['entry_same_epoch']!=1 or not s['n']:
        raise ValueError('missing complete initial baseline/sums')
    if b['acquisition']['dma'] and b['acquisition']['trigger_us']!=s['trigger_us']:
        raise ValueError('baseline/powered DMA cadence mismatch')
    offsets=[r['sum']/r['n']-2048 for r in b['channels'][:3]]
    means=[v/s['n'] for v in s['signed_raw_sums']]
    residual=[x-y for x,y in zip(means,offsets)]
    return dict(powered_centered_mean_counts=means,baseline_centered_mean_counts=offsets,
                residual_counts=residual,sum_residual_counts=sum(residual),scans=s['n'],
                calibrated_current=False,stationarity_verified=False,drift_verified=False,
                initial_staging_epoch_matched=True,
                baseline_acquisition=b['acquisition'],amps=None)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path)
    print(json.dumps(report(p.parse_args().capture.read_text()),indent=2))
