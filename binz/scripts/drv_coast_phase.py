"""Interval-censored coast phase model; never a motor-control calibration.

Assumes balanced sinusoidal phase voltages,neutral comparator sign,constant
frequency over the selected early window,valid transitions/no aliasing,plus
the caller's explicit timing uncertainty. Grid fits are NOT confidence bounds.
Stop-to-coast offset is not recorded in E210,so do not apply the phase difference
as a commutation correction. No hardware access or output authority.
"""
import argparse
import json
import math
from pathlib import Path
import re
from drv_capture import parse_dump
from drv_driven_run import verify,coast_origin

def fits(samples,hz_values=range(150,261),phases=range(360),direction=1):
    """Samples:lower_us,upper_us,phase0..2,rawCOMPlevel. A raw high means Vphase<Vneutral."""
    if direction not in (-1,1) or not samples: raise ValueError('invalid model input')
    result=[]
    for hz in hz_values:
        if hz<=0: raise ValueError('positive frequency required')
        for low,high,phase,_ in samples:
            if high<low or phase not in (0,1,2) or (high-low)*hz>=500000:
                raise ValueError('read interval spans a half-cycle or is invalid')
        for theta in phases:
            valid=True
            for low,high,phase,level in samples:
                # Interval is shorter than a half-cycle: matching either
                # endpoint suffices; otherwise no matching interior sign.
                def raw(us):
                    angle=math.radians(theta+120*phase+direction*hz*360*us/1e6)
                    return math.sin(angle)<0
                if raw(low)!=level and raw(high)!=level:
                    valid=False;break
            if valid: result.append((hz,theta))
    return result

def analyze(text,uncertainty_us):
    if not 0<=uncertainty_us<=250: raise ValueError('uncertainty must be0..250us')
    state=verify(text) # actual observer completion,CRC,feedback,finaloff first
    coast=parse_dump(text)[2]
    offsets=re.findall(r'^COASTCOMP row=(\d+) a_us=(\d+) b_us=(\d+) c_us=(\d+) from_scan_start=1$',text,re.M)
    if len(offsets)!=32 or [int(o[0]) for o in offsets]!=list(range(32)):
        raise ValueError('missing early comparator brackets')
    samples=[]
    # Fixed first12rows (~6ms),not a window selected to make the fit pass.
    for row,offset in zip(coast[:12],offsets[:12]):
        a,b,c=map(int,offset[1:])
        if not 0<a<b<c<1000 or 'elapsed_us' not in row: raise ValueError('invalid read timing')
        for phase in range(3):
            samples.append((row['elapsed_us']+[0,a,b][phase]-uncertainty_us,
                row['elapsed_us']+[a,b,c][phase]+uncertainty_us,phase,bool(row['flags']&(2<<phase))))
    if len(samples)!=36: raise ValueError('12complete rows required')
    result=[]
    origin=coast_origin(text)
    commanded=((state['theta']+state['rate']*state['stop_us'])&0xffffffff)*360/2**32
    for direction in [1,-1]:
        grid=fits(samples,direction=direction)
        relative=set()
        if origin:
            for hz,phase in grid:
                bounds=[phase-direction*hz*360*delay/1e6-commanded for delay in origin]
                for degree in range(math.floor(min(bounds)),math.ceil(max(bounds))+1):
                    relative.add((degree+180)%360-180)
        result.append(dict(direction=direction,feasible_grid_points=len(grid),
            frequency_grid_hz=sorted({hz for hz,_ in grid}),phase_grid_degrees=sorted({p for _,p in grid}),
            relative_phase_at_stop_grid_degrees=sorted(relative)))
    return dict(model='balanced_sine_constant_frequency_first12coast_rows',read_uncertainty_us=uncertainty_us,
        grid_step_hz=1,grid_step_degrees=1,directions=result,
        commanded_phase_at_stop_degrees=commanded,stop_to_coast_delay_us=origin,
        stop_to_coast_offset_measured=origin is not None,phase_correction_authorized=False,lock_proven=False,
        caveat='Conditional model feasibility,not confidence bounds or control calibration. Relative phase is only supplied with a measured clock-origin bracket; constant-frequency back-extrapolation remains a model assumption.')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('capture',type=Path);p.add_argument('--uncertainty-us',required=True,type=int)
    args=p.parse_args();print(json.dumps(analyze(args.capture.read_text(),args.uncertainty_us),indent=2))
