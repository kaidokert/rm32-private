"""Guarded20ms under-drive capture/replay. Completion is NOT BEMF lock."""
import argparse
import base64
import json
from pathlib import Path
import re
import struct
import zlib
from drv_capture import live_capture,verify_off,parse_dump

def records(text,label,width):
    result=[]
    for body in re.findall(r'^'+label+r' ([^\r\n]+)$',text,re.M):
        raw=base64.a85decode(body.encode('ascii'))
        if len(raw)!=width*2+4 or zlib.crc32(raw[:-4])!=struct.unpack('<I',raw[-4:])[0]:
            raise ValueError(label+' length/CRC mismatch')
        result.append(struct.unpack('<'+'H'*width,raw[:-4]))
    return result

def coast_origin(text):
    if 'CB85 ' not in text and 'DRIVENCOAST ' not in text: return None
    if text.count('DRIVENCOAST fields=valid,delay_min_us,delay_max_us t17_brackets_coast_origin=1')!=1:
        raise ValueError('invalid coast origin provenance')
    rows=records(text,'CB85',3)
    if len(rows)!=1 or rows[0][0]!=1 or not 0<=rows[0][1]<=rows[0][2]<=1000 or rows[0][2]-rows[0][1]>20:
        raise ValueError('invalid shutdown-to-coast clock bracket')
    return rows[0][1:]

def seed_window(text, rows, exclude_partial):
    """Select a fresh contiguous window; never splice intervals across a gap."""
    source = [r for r in rows if r[0] != 0] if exclude_partial else rows
    markers = re.findall(r'^DRIVENSEEDRESTART ([^\r\n]+)', text, re.M)
    timing = re.findall(r'^SEEDTIMING ([^\r\n]+)', text, re.M)
    discarded = 0
    if timing:
        tm = re.fullmatch(r'discarded_fault=(0|3) max_restarts=1 fresh_intervals=12 original_deadline=1 experimental=1', timing[0])
        if len(timing)!=1 or not tm or not markers:
            raise ValueError('invalid timing-restart provenance')
        discarded=int(tm[1])
    if not markers:
        return source[:13]
    match = re.fullmatch(r'count=([01]) anchor_epoch=(\d+) max_restarts=1 fresh_intervals=12 original_deadline=1', markers[0])
    if len(markers) != 1 or not match or not exclude_partial:
        raise ValueError('invalid seed restart provenance')
    count, anchor = map(int, match.groups())
    if not count:
        if anchor or discarded: raise ValueError('unused seed restart has anchor/fault')
        return source[:13]
    for index, (a, b) in enumerate(zip(source, source[1:]), 1):
        gap = b[0] - a[0]
        delta=2*(b[2]-a[2])
        timing_gap=discarded==3 and gap==1 and b[1]==a[1]%6+1 and delta>2000
        if gap != 1 or b[1] != a[1] % 6 + 1 or timing_gap:
            if (index >= 13 or (not timing_gap and (discarded or not 2 <= gap <= 6)) or b[0] != anchor
                    or b[1] != (a[1] - 1 + gap) % 6 + 1
                    or not 0 < delta <= 12000 or 2 * b[2] > 40000):
                raise ValueError('restart does not match first admissible gap')
            if timing_gap and not max(0,2*(b[2]-a[3])-2)<=b[4]<=2*(b[3]-a[2])+2:
                raise ValueError('discarded interval lacks timer corroboration')
            prefix = source[:index]
            fast = 'RUNLIMIT cycle_min_us=3333 event_min_us=277 ' in text
            minimum, cycle_min = (554, 6666) if fast else (666, 8000)
            if 'RUNLIMIT cycle_min_us=3226 event_min_us=268 ' in text:
                minimum, cycle_min = 536, 6452
            if 'RUNLIMIT cycle_min_us=3031 event_min_us=252 ' in text:
                minimum, cycle_min = 504, 6062
            elif 'RUNLIMIT cycle_min_us=2986 event_min_us=248 ' in text:
                minimum, cycle_min = 496, 5972
            elif 'RUNLIMIT cycle_min_us=2942 event_min_us=245 ' in text:
                minimum, cycle_min = 490, 5884
            elif 'RUNLIMIT cycle_min_us=2899 event_min_us=241 ' in text:
                minimum, cycle_min = 482, 5798
            elif any(f'RUNLIMIT cycle_min_us={floor} event_min_us=238 ' in text for floor in (2858,2778,2500,2223)):
                minimum, cycle_min = 476, 5716
            elif 'RUNLIMIT cycle_min_us=3125 event_min_us=260 ' in text:
                minimum, cycle_min = 520, 6250
            from drv_seed_profile import seed400
            from drv_seed_profile import profile as seed_profile
            selected=seed_profile(text)
            if selected:cycle_min,minimum,_=selected
            if (any(r[3]-r[2]>20 for r in source[:index+1])
                    or any(y[0]!=x[0]+1 or y[1]!=x[1]%6+1 for x,y in zip(prefix,prefix[1:]))
                    or any(not minimum<=2*(y[2]-x[2])<=2000 for x,y in zip(prefix,prefix[1:]))
                    or any(not cycle_min<=2*(prefix[j+6][2]-prefix[j][2])<=12000
                           for j in range(max(0,len(prefix)-6)))):
                raise ValueError('restart after an already-fatal timing fault')
            return source[index:index+13]
    raise ValueError('restart without a recorded gap')

def irq_observation(text,commands,stop):
    if not re.search(r'^(?:DRIVENIRQ|DI85|DI85FIELDS) ',text,re.M): return {}
    headers=re.findall(r'^DRIVENIRQ calls=(\d+) accepts=(\d+) max_us=(\d+) rate_peak=(\d+) rate_limit=64 fixed_average_ticks=1666 filter_reads=12 raw_inverted=1 handoff_authority=0 masked=1$',text,re.M)
    fields='DI85FIELDS epoch,step,before_us,after_us,interval_half_us,requested_arr,previous_accept_exists'
    if len(headers)!=1 or text.count(fields)!=1: raise ValueError('missing IRQ observer provenance')
    calls,n,cost,peak=map(int,headers[0]);rows=records(text,'DI85',7)
    rate_report_only=text.count('STARTUPRATE report_only=1 tick_watchdog=1 handler_overrun_stop=1')==1
    if len(rows)!=n or not 0<=n<=26 or calls<n or (peak>64 and not rate_report_only) or cost>50:
        raise ValueError('IRQ observer count/timing/rate failed')
    previous=None
    for i,(epoch,step,before,after,interval,arr,has_previous) in enumerate(rows):
        if epoch>=len(commands) or step!=commands[epoch][1] or has_previous!=int(i>0) or arr!=417:
            raise ValueError('IRQ observer epoch/metadata failed')
        end=commands[epoch+1][2] if epoch+1<len(commands) else stop
        if not commands[epoch][2]<=before<=after<end or after-before>cost or interval<=833:
            raise ValueError('IRQ observer physical time/gate failed')
        if previous:
            pe,pb,pa=previous
            if epoch<=pe or before<=pa:
                raise ValueError('IRQ observer duplicated/reordered epoch')
            # The live interval sample/reset both occur inside ISR brackets.
            # Allow one half-us timer tick of endpoint quantization on each side.
            if not max(0,2*(before-pa)-2)<=interval<=2*(after-pb)+2:
                raise ValueError('IRQ interval disagrees with acquisition brackets')
        previous=(epoch,before,after)
    seed_info={}
    if re.search(r'^(?:DRIVENSEED|DS85) ',text,re.M):
        header='DRIVENSEED fields=ready,step,edge_lo,edge_hi,average,intervals,cycles,cycle_min,cycle_max,fault conservative_bracket_start=1 handoff_authority=0'
        seeds=records(text,'DS85',10)
        seed_headers=re.findall(r'^DRIVENSEED ([^\n]+)$',text,re.M)
        if len(seed_headers)!=1 or len(seeds)!=1: raise ValueError('missing driven seed provenance')
        actual='DRIVENSEED '+seed_headers[0]
        if actual not in (header,header+' partial_epoch_excluded=1'):
            raise ValueError('unknown driven seed admission policy')
        exclude_partial=actual.endswith(' partial_epoch_excluded=1')
        ready,step,lo,hi,average,intervals,cycles,cmin,cmax,fault=seeds[0]
        if ready not in (0,1) or intervals>12 or cycles>7: raise ValueError('invalid seed state')
        initial=seed_window(text,rows,exclude_partial)
        if ready:
            if len(initial)!=13 or fault or intervals!=12 or cycles!=7:
                raise ValueError('seed without complete qualification')
            if any(r[3]-r[2]>20 for r in initial) or initial[-1][2]*2>40000:
                raise ValueError('seed bracket/deadline failed')
            gaps=[2*(b[2]-a[2]) for a,b in zip(initial,initial[1:])]
            cycle_times=[2*(initial[j+6][2]-initial[j][2]) for j in range(7)]
            if (any(b[0]!=a[0]+1 for a,b in zip(initial,initial[1:]))
                or not all(666<=v<=2000 for v in gaps)
                or not all(8000<=v<=12000 for v in cycle_times)
                or (step,lo|(hi<<16),average,cmin,cmax)!=(initial[-1][1],initial[-1][2]*2,sum(gaps)//12,min(cycle_times),max(cycle_times))):
                raise ValueError('seed disagrees with live interval sequence')
        elif step or lo or hi or average: raise ValueError('unqualified seed has values')
        seed_info={'driven_seed_ready':bool(ready),'driven_seed_fault':fault,
                   'driven_seed_edge_tick':lo|(hi<<16),'driven_seed_average':average}
    return {'irq_calls':calls,'irq_accepts':n,'irq_max_us':cost,'irq_rate_peak':peak,
            'irq_observation_only':True,'irq_rate_report_only':rate_report_only,**seed_info}

def pwm_comp(text,commands,*,transfer=False):
    headers=re.findall(r'^PWMCOMP n=(\d+) target=(192|320) flags=(\d+) stopped=1 source_comp2_csr=1 handoff_authority=0$',text,re.M)
    if len(headers)!=1: raise ValueError('missing complete PWM-synchronous comparator provenance')
    n,target,flags=map(int,headers[0]);raw=records(text,'PC85',3);bounds=records(text,'PD85',3)
    # Fixed256-word DMA: HTIF/GIF appear only after128 transfers. A fresh
    # handoff can stop before halfway; TCIF/TEIF remain forbidden either way.
    expected_flags=0 if transfer and n<128 else 5
    if not (64 if transfer else 180)<=n<=240 or flags!=expected_flags or len(raw)!=n or [r[0] for r in raw]!=list(range(n)):
        raise ValueError('finite PWM-DMA count/flags/records failed')
    if len(bounds)!=len(commands) or bounds[0]!=(0,0,0): raise ValueError('missing PWM command boundaries')
    values=[lo|hi<<16 for _,lo,hi in raw];candidates=0;stable=0;opposite_samples=0;raw_candidates=0;candidate_epochs=[]
    def raw_candidate(values,expected):
        opposite=0;count=0
        for value in values:
            if bool(value&(1<<30))!=expected: opposite=min(2,opposite+1);count=0
            elif opposite<2: opposite=0
            else:
                count+=1
                if count>=2: return 1
        return 0
    for i,(epoch,before,after) in enumerate(bounds):
        if epoch!=i or not 0<=before<=after<=n or (i and before<bounds[i-1][2]):
            raise ValueError('invalid DMA command bracket')
        end=bounds[i+1][1] if i+1<len(bounds) else n
        step=commands[i][1];mux=[8,6,7,8,6,7][step-1]
        if after>end: raise ValueError('overlapping DMA command brackets')
        stable+=end-after
        raw_candidates+=raw_candidate(values[after+2:end],bool(step&1))
        opposite=0;count=0;emitted=False
        for j,value in enumerate(values[after:end]):
            if value&1==0 or value&(1<<15) or (value>>4)&15!=mux:
                raise ValueError('PWM sample comparator configuration/phase mismatch')
            # Exclude the first two100us-spaced samples conservatively. This
            # is offline diagnostic replay,not a timestamped seed/production ZC.
            if j<2: continue
            level=not bool(value&(1<<30));expected=bool(step&1)
            opposite_samples+=int(level!=expected)
            if emitted: continue
            if level!=expected: opposite=min(2,opposite+1);count=0
            elif opposite<2: opposite=0
            else:
                count+=1
                if count>=2: candidates+=1;emitted=True;candidate_epochs.append(i)
    return {'pwm_target':target,'pwm_dma_samples':n,'pwm_stable_epoch_samples':stable,
            'pwm_opposite_samples_after_blank':opposite_samples,'pwm_offline_candidate_sectors':candidates,
            'pwm_candidate_epochs':candidate_epochs,'pwm_raw_polarity_candidate_sectors':raw_candidates}

def verify(text,*,transfer=False):
    text=text.replace('\r','')
    summaries=re.findall(r'^DRIVEOBS reason=([^\n]+)$',text,re.M)
    if len(summaries)!=1 or text.count('DRIVEOBS END')!=1:
        raise ValueError('one complete driven result required (startup may have refused)')
    s={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)','reason='+summaries[0])}
    duration=s['stop_us']-s['start_us']
    completion=(s['reason']==22 and 8000<=duration<=20200) if transfer else (s['reason']==2 and 20000<=duration<=20200)
    if not (completion
            and s['handoff_authority']==0 and s['disabled']==1 and s['rate']==858993
            and 40<=s['duty_tenths']<=62 and s['tick_max_us']<=50
            and s['sector_max_us']<=100 and s['late_max_us']<=50 and s['age_max_us']<=1000):
        raise ValueError('guarded drive completion/timing contract failed: '+str(s))
    q=records(text,'DQ85',4);a=records(text,'DA85',7);c=records(text,'DC85',3)
    if len(q)!=s['reads'] or not (100 if transfer else 350)<=len(q)<=416 or len(a)!=s['scans'] or not (10 if transfer else 50)<=len(a)<=256:
        raise ValueError('missing comparator/ADC records')
    if len(c)!=s['commands']+1 or not (12 if transfer else 23)<=s['commands']<=25:
        raise ValueError('missing physical command records')
    if not s['start_us']<=c[0][2]<s['first_us'] or not s['first_us']<=c[1][2]<=s['first_us']+100:
        raise ValueError('initial partial-sector timing mismatch')
    for i,(epoch,step,t) in enumerate(c):
        if epoch!=i or not 1<=step<=6 or not s['start_us']<=t<s['stop_us']:
            raise ValueError('invalid command epoch/time')
        if i and (step!=c[i-1][1]%6+1 or not 500<=t-c[i-1][2]<=1100 and i>1):
            raise ValueError('command order/timing mismatch')
    # Independent replay of diagnostic opposite-level arming/persistence.
    # This checks the instrument,not production AM32 lock or independent qZC.
    events=[];last=None;epoch=None;opposite=0;expected_count=0;emitted=False;onset=None
    for t,pwm,flags,e in q:
        step=flags&7;level=bool(flags&8);status=(flags>>4)&3;bracket=flags>>8
        if flags&0xc0 or pwm>=6400 or e>=len(c) or step!=c[e][1] or not c[e][2]<=t<s['stop_us']:
            raise ValueError('invalid comparator record')
        if e+1<len(c) and t>=c[e+1][2]: raise ValueError('sample uses stale physical epoch')
        if last is not None and not 0<t-last<=100: raise ValueError('comparator sample gap')
        if e!=epoch: epoch=e;opposite=0;expected_count=0;emitted=False;onset=None
        rejected=bracket>2 or t-c[e][2]<200
        event=False
        if rejected: opposite=0;expected_count=0;onset=None
        elif not emitted:
            expected=bool(step&1)
            if level!=expected:
                opposite=min(2,opposite+1);expected_count=0;onset=None
            elif opposite>=2:
                if onset is None: onset=t
                expected_count+=1
                if expected_count>=2:
                    event=True;emitted=True;events.append((step,onset,t+bracket))
            else:
                opposite=0
        if status!=(2 if rejected else int(event)): raise ValueError('candidate replay disagrees with firmware')
        last=t
    if len(events)!=s['candidates']: raise ValueError('candidate count mismatch')
    last=None
    for t,age,pa,pb,pc,bus,vref in a:
        if (last is not None and t<=last) or age>s['age_max_us'] or not 0<age<=1000:
            raise ValueError('ADC age/order mismatch')
        if t<s['start_us']: raise ValueError('ADC before drive start')
        if t+age>s['stop_us'] or not all(848<=v<=3248 for v in [pa,pb,pc]) or bus<8400 or not 0<vref<4095:
            raise ValueError('electrical feedback contract failed')
        last=t
    marker=text.rfind('FINALOFF\n')
    if marker<0 or 'COAST END' not in text[:marker]: raise ValueError('missing coast/final-off evidence')
    verify_off(text[marker:].encode())
    stack=re.search(r'STACK[^\n]*span=(\d+)[^\n]*untouched=(\d+)',text[marker:])
    if not stack or int(stack[1])<4096 or int(stack[2])<512:
        raise ValueError('missing or insufficient stack headroom')
    longest=0;run=0;previous=None
    for step,onset,_ in events:
        run=run+1 if previous and step==previous[0]%6+1 and 333<=onset-previous[1]<=1000 else 1
        longest=max(longest,run);previous=(step,onset)
    pwm=pwm_comp(text,c,transfer=transfer) if re.search(r'^(?:PWMCOMP|PC85|PD85) ',text,re.M) else {}
    if transfer:
        if re.findall(r'^DRIVENCOAST (.*)$',text,re.M)!=['unavailable=1 acquisition_release_not_final_stop=1'] or records(text,'CB85',3):
            raise ValueError('handoff must not claim acquisition release is final coast origin')
        origin=None
    else:
        origin=coast_origin(text)
    phase=re.findall(r'^DRIVEPHASE applied_degrees=(-?\d+) commanded_only=1$',text,re.M)
    if len(phase)>1 or (phase and int(phase[0]) not in (-30,0,30,60)): raise ValueError('invalid applied phase')
    if pwm and pwm['pwm_target']+32>=6400*s['duty_tenths']//1000:
        raise ValueError('PWM sample latency margin extends beyond ON pulse')
    irq=irq_observation(text,c,s['stop_us'])
    align=re.findall(r'^DRIVEALIGN (.*)$',text,re.M)
    alignment={}
    if align:
        match=re.fullmatch(r'initial_remaining_us=(\d+) requested_wait_us=(\d+) actual_wait_us=(\d+) gates_disabled_during_wait=1 elapsed_phase=1',align[0])
        if len(align)!=1 or not match: raise ValueError('invalid initial alignment provenance')
        initial,requested,actual=map(int,match.groups())
        if (not 1<=initial<=1000 or requested!=(initial if initial<100 else 0)
                or not 0<=actual<=requested+4 or (requested==0 and actual!=0)
                or s['start_us']>500 or s['first_us']-s['start_us']<100):
            raise ValueError('initial alignment exceeded bounded admission contract')
        alignment={'initial_remaining_us':initial,'alignment_wait_us':actual}
    if transfer and (not pwm or not irq.get('driven_seed_ready')):
        raise ValueError('transfer acquisition requires PWM records and a qualified live seed')
    return {**s,**pwm,**irq,**alignment,'phase_shift_degrees':int(phase[0]) if phase else 0,'stop_to_coast_delay_us':origin,'longest_ordered_candidate_edges':longest,'peak_abs_raw':max(abs(v-2048) for row in a for v in row[2:5]),
            'bus_min_mv':min(row[5] for row in a),'bemf_lock_proven':False}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path);ap.add_argument('--infile',type=Path)
    ap.add_argument('--pwm-target',type=int,choices=[192,320],default=192)
    ap.add_argument('--drive-duty',type=int,choices=range(40,63),default=62,metavar='40..62')
    ap.add_argument('--phase-shift',type=int,choices=[-30,0,30,60],default=0)
    args=ap.parse_args()
    if bool(args.out)==bool(args.infile): ap.error('select --out for live capture OR --infile for replay')
    if args.out and args.pwm_target+32>=6400*args.drive_duty//1000:
        ap.error('sample plus timing margin must fit inside the selected ON pulse')
    if args.infile: text=args.infile.read_text()
    else:
        # Reserve before touching hardware; all partial attempts retained.
        with args.out.open('xb'): pass
        text=live_capture('COM41',115200,50,62,False,None,raw_path=args.out,
            step_targets=tuple(range(60,201,10)),catch_duty=62,capture_stride=19,driven=True,drive_sample=args.pwm_target,drive_duty=args.drive_duty,drive_phase=args.phase_shift)
    parse_dump(text)
    result=verify(text)
    if args.out and (result['duty_tenths']!=args.drive_duty or result['pwm_target']!=args.pwm_target or result['phase_shift_degrees']!=args.phase_shift):
        raise ValueError('applied driven settings do not match this experiment')
    print(json.dumps(result,indent=2))

if __name__=='__main__': main()
