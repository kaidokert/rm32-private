"""Live-armed baseline, optionally one query and a70->69 duty transition.

Reuse the established startup pacing and strict handoff verifier. This is NOT
a fixed-setting qualification when --change69 is used; retain failed startup.
"""
from pathlib import Path
import argparse
import re
import drv_capture
from drv_driven_handoff import verify, verify_seed_timing, verify_cycle360, verify_cycle400, verify_cycle450
from drv_role_check import verify_mode


def verify_running_profile(text,cycle400=False,cycle450=False,event100=False):
    if cycle400 and cycle450:raise ValueError('choose one running profile')
    verify_cycle360(text,not (cycle400 or cycle450))
    verify_cycle400(text,cycle400)
    verify_cycle450(text,cycle450,event100)

def lean_exploration_report(text):
    """Retain lean controller output; never infer lock from empty recorders."""
    if text.count('FINALOFF\n')!=1:raise ValueError('missing unique finaloff boundary')
    drv_capture.verify_off(text.split('FINALOFF\n')[1].encode())
    markers=re.findall(r'^LEANCORE r1 recorder=0 comp_max=0 com_max=0 control_progress=1 lean_irq=1\s*$',text,re.M)
    return dict(exploration_only=True,qualification=False,outputs_off_verified=True,
                lean_summary_seen=len(markers)==1,
                controller_output=[line for line in text.splitlines() if line.startswith(
                    ('TIMING ','COASTREF ','CORECHECK ','CORESTATE ','POWERGUARD ','DRIVEN ','RUN refused:'))])

def target_dwell_report(text, expected_duty, stop_us, minimum_ms):
    rows=re.findall(
        r'^LIVEACK target_tenths=(\d+) seen=(\d+) accepted_us=(\d+) stop_us=(\d+) age_us=(\d+) same_powered_segment=1 ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1\s*$',
        text,re.M)
    matches=[tuple(map(int,row)) for row in rows if int(row[0])==expected_duty]
    if len(matches)!=1:
        raise RuntimeError(f'missing unique MCU target ACK stamp: {expected_duty=} {matches=}')
    duty,seen,accepted,reported_stop,age=matches[0]
    if seen!=1 or accepted==0:
        raise RuntimeError(f'incoherent MCU target dwell stamp: {matches[0]} {stop_us=}')
    if reported_stop==stop_us and age==stop_us-accepted:
        dwell=age
        source='powered_timer_stop'
    elif reported_stop==0 and age==0:
        # A normal foreground deadline stop leaves powered_timer's stopped
        # snapshot at zero. TRACKSTOP is the last real accepted-event time,
        # hence a conservative lower bound on time spent at target duty.
        power=re.findall(r'^POWERPATH reason=(\d+) stop_us=(\d+) .*disabled=(\d+)\s*$',text,re.M)
        events=re.findall(r'^TRACKSTOP event_fault=(\d+) last_event_us=(\d+) .*$',text,re.M)
        coast=re.findall(r'^DRIVENENTRY refusal=(\d+) .* coast_stop=(\d+) .* postrun_only=1\s*$',text,re.M)
        if power!=[('0','0','1')] or len(events)!=1 or events[0][0]!='0' or len(coast)!=1 or coast[0][1]!='1':
            raise RuntimeError(f'foreground target dwell lacks normal-stop evidence: {power=} {events=} {coast=}')
        event_us=int(events[0][1])
        if not stop_us-1000<=event_us<=stop_us+1000 or event_us<accepted:
            raise RuntimeError(f'foreground target dwell event is not near stop: {event_us=} {stop_us=} {accepted=}')
        dwell=event_us-accepted
        source='last_accepted_event_lower_bound'
    else:
        raise RuntimeError(f'incoherent MCU target dwell stamp: {matches[0]} {stop_us=}')
    if dwell<minimum_ms*1000:
        raise RuntimeError(f'target dwell too short: {dwell=} required_us={minimum_ms*1000}')
    return dict(duty_tenths=duty,accepted_us=accepted,stop_us=stop_us,dwell_us=dwell,source=source)

def lean_hold_report(text, expected_duty, expected_ms, minimum_target_ms=None):
    """Require an uninterrupted powered deadline on a lean protected image."""
    if not 40 <= expected_duty <= 500:
        raise ValueError('hold target outside live envelope')
    transfer=re.findall(r'^DRIVETRANSFER result=(\d+) zero_not_attempted=1 two_refused=1\s*$',text,re.M)
    power=re.findall(r'^POWERPATH reason=(\d+) stop_us=(\d+) .*disabled=(\d+)\s*$',text,re.M)
    coast=re.findall(r'^DRIVENENTRY refusal=(\d+) .* coast_stop=(\d+) .* fly_seeded=(\d+) postrun_only=1\s*$',text,re.M)
    last_event=re.findall(r'^TRACKSTOP event_fault=(\d+) last_event_us=(\d+) .*$',text,re.M)
    commits=re.findall(r'^POWERCOMMITS applied=(\d+)\s*$',text,re.M)
    fastbus=re.findall(r'^FASTBUS .* tripped=(\d+) same_wake=1\s*$',text,re.M)
    fold=re.findall(r'^CURRENTFOLDBACK count=(\d+) .*$',text,re.M)
    if transfer!=['1'] or len(power)!=1 or power[0][2]!='1' or len(coast)!=1 or coast[0][0]!='0' or coast[0][2]!='1':
        raise RuntimeError(f'hold did not enter one protected powered window: {transfer=} {power=} {coast=}')
    expected=expected_ms*1000
    if power[0][0]=='2':
        elapsed=int(power[0][1])
        if coast[0][1] not in ('1','7') or not expected<=elapsed<=expected+1000:
            raise RuntimeError(f'guard deadline mismatch: {elapsed=} {expected=} {coast=}')
    elif power[0][:2]==('0','0') and coast[0][1]=='1':
        # The foreground loop can observe the requested deadline just before
        # TIM6's guard does. Both are normal stops; require a recent real ZC.
        if len(last_event)!=1 or last_event[0][0]!='0':
            raise RuntimeError(f'foreground deadline lacks healthy event: {last_event=}')
        elapsed=int(last_event[0][1])
        if not expected-1000<=elapsed<=expected+1000:
            raise RuntimeError(f'foreground deadline event age mismatch: {elapsed=} {expected=}')
    else:
        raise RuntimeError(f'hold did not finish a normal powered deadline: {power=} {coast=}')
    if len(commits)!=1 or int(commits[0])==0 or fastbus!=['0'] or fold!=['0']:
        raise RuntimeError(f'hold progress/protection failed: {commits=} {fastbus=} {fold=}')
    if re.search(r'^NORMALRESTART result=1\b',text,re.M):
        raise RuntimeError('hold used automatic normal restart')
    if not re.search(rf'^D={expected_duty:04X} I=[0-9A-F]{{4}} \s*$',text,re.M):
        raise RuntimeError(f'hold target {expected_duty} not ACKed')
    result=dict(duty_tenths=expected_duty,powered_us=elapsed,commits=int(commits[0]),
                fastbus=0,foldback=0,uninterrupted=True)
    if minimum_target_ms is not None:
        result['target_dwell']=target_dwell_report(text,expected_duty,elapsed,minimum_target_ms)
    return result

def normal_restart_compact_report(text, expected_resume=None, expected_steps=None):
    """Verify a compact reverse image's two real powered windows and deadline."""
    def one(pattern, label):
        matches = re.findall(pattern, text, re.M)
        if len(matches) != 1:
            raise RuntimeError(f'{label}: expected one report, got {matches}')
        return matches[0]
    first = one(r'^NORMALRESTART result=(\d+) first_reason=(\d+) first_stop_us=(\d+) first_commits=(\d+) original_deadline=1 flying_seed=0 one_shot=1\s*$', 'first stop')
    second = one(r'^NORMALRESTART2 handoff=(\d+) final_drive_reason=(\d+) final_power_reason=(\d+) final_power_stop_us=(\d+) settle_us=(\d+) settings_replayed=(\d+) outputs_disabled=(\d+)\s*$', 'second outcome')
    resume = one(r'^NORMALRESTART3 resume_target=(\d+) resume_applied=(\d+) resume_steps=(\d+) step_tenths=50 period_us=2000000 foreground_only=1\s*$', 'duty restore')
    deadline = one(r'^NORMALRESTART4 planned_remaining_us=(\d+) actual_second_power_us=(\d+) window_rounding_us=1000 postrun_only=1\s*$', 'deadline')
    power = one(r'^POWERPATH reason=(\d+) stop_us=(\d+) .*disabled=1\s*$', 'power stop')
    transfer = one(r'^DRIVETRANSFER result=(\d+) zero_not_attempted=1 two_refused=1\s*$', 'fresh transfer')
    if first[:2] != ('1', '8') or int(first[2]) < 1_000_000 or int(first[3]) == 0:
        raise RuntimeError(f'first tracking stop invalid: {first}')
    if second[0] != '1' or second[2] != '2' or second[5:] != ('1', '1') or int(second[4]) < 1_000_000:
        raise RuntimeError(f'second handoff/stop invalid: {second}')
    planned, actual = map(int, deadline)
    if planned < actual or planned - actual > 1000 or actual != int(second[3]) or power != ('2', str(actual)):
        raise RuntimeError(f'second deadline invalid: planned={planned} actual={actual} power={power}')
    if transfer[0] != '1':
        raise RuntimeError(f'fresh transfer invalid: {transfer}')
    if expected_resume is not None and (int(resume[0]), int(resume[1])) != (expected_resume, expected_resume):
        raise RuntimeError(f'restored duty invalid: {resume}')
    if expected_steps is not None and int(resume[2]) != expected_steps:
        raise RuntimeError(f'restore step count invalid: expected={expected_steps} actual={resume}')
    return dict(first_tracking_stop_us=int(first[2]), remaining_powered_us=actual,
                planned_remaining_us=planned, resume=tuple(map(int, resume)), fresh_transfer=True)


def normal_restart_report(text,require_outcome=False,expected_resume=None,foldback_after_restore=False):
    """Require a completed second powered window, not merely a launched restart."""
    reports=re.findall(r'^NORMALRESTART result=(\d+) first_reason=(\d+) first_stop_us=(\d+) first_commits=(\d+) original_deadline=1 flying_seed=0 one_shot=1\s*$',text,re.M)
    if len(reports)!=1 or reports[0][0]!='1' or reports[0][1]!='8':
        raise RuntimeError(f'normal restart not launched after tracking loss: {reports}')
    coast=re.findall(r'^COASTREF stop=(\d+) max_us=(\d+) .*$',text,re.M)
    powered=re.findall(r'^POWERPATH reason=(\d+) stop_us=(\d+) .*disabled=1\s*$',text,re.M)
    transfer=re.findall(r'^DRIVEX result=(\d+) power_reason=(\d+) fresh_transfer=1\s*$',text,re.M)
    # Foreground may observe the same deadline first (1) or find that the
    # powered owner already ended it (7). POWERPATH below is authoritative.
    if len(coast)!=1 or coast[0][0] not in ('1','7'):
        raise RuntimeError(f'normal restart did not retain its full deadline: {coast}')
    expected=int(coast[0][1])
    outcomes=re.findall(r'^NORMALRESTART2 handoff=(\d+) final_drive_reason=(\d+) final_power_reason=(\d+) final_power_stop_us=(\d+) settle_us=(\d+) settings_replayed=(\d+) outputs_disabled=(\d+)\s*$',text,re.M)
    if require_outcome and len(outcomes)!=1:
        raise RuntimeError(f'missing unique second-attempt outcome: {outcomes}')
    if outcomes:
        if len(outcomes)!=1 or outcomes[0][0]!='1' or outcomes[0][2]!='2' or outcomes[0][5:]!=('1','1'):
            raise RuntimeError(f'normal restart second-attempt outcome failed: {outcomes}')
        if abs(int(outcomes[0][3])-expected)>20 or int(outcomes[0][4])<1_000_000:
            raise RuntimeError(f'normal restart outcome deadline/settle failed: expected={expected} outcome={outcomes[0]}')
    if len(transfer)!=1 or transfer[0] != ('1','2'):
        raise RuntimeError(f'normal restart did not complete a fresh powered handoff: {transfer}')
    if len(powered)!=1 or powered[0][0]!='2' or abs(int(powered[0][1])-expected)>20:
        raise RuntimeError(f'normal restart did not complete remaining powered window: expected={expected} reports={powered}')
    resume=re.findall(r'^NORMALRESTART3 resume_target=(\d+) resume_applied=(\d+) resume_steps=(\d+) step_tenths=50 period_us=2000000 foreground_only=1\s*$',text,re.M)
    if expected_resume is not None:
        if len(resume)!=1 or int(resume[0][0])!=expected_resume or int(resume[0][1])!=expected_resume:
            raise RuntimeError(f'normal restart did not restore requested duty {expected_resume}: {resume}')
        if expected_resume!=70 and int(resume[0][2])==0 and not foldback_after_restore:
            raise RuntimeError(f'normal restart claimed restore without a step: {resume}')
    return dict(first_tracking_stop_us=int(reports[0][2]),
                remaining_powered_us=int(powered[0][1]),fresh_transfer=True,
                deadline_complete=True,resume=(tuple(map(int,resume[0])) if resume else None))

def current_foldback_report(text,requested):
    """Validate the opt-in foreground foldback summary and effective ceiling."""
    fixed=re.findall(r'^CURRENTFOLDBACK count=(\d+) last_duty_tenths=(\d+) step_tenths=(10|50) release=none first_over_warning=1 (?:unacknowledged_)?second_over_stop=1 foreground_writer=1\s*$',text,re.M)
    adaptive=re.findall(r'^CURRENTFOLDBACK count=(\d+) last_duty_tenths=(\d+) last_step_tenths=(\d+) step_policy=severity10_50 release=none first_over_warning=1 unacknowledged_second_over_stop=1 foreground_writer=1\s*$',text,re.M)
    if len(fixed)+len(adaptive)!=1:
        raise RuntimeError(f'missing unique current-foldback summary: fixed={fixed} adaptive={adaptive}')
    count,last,step=map(int,(fixed or adaptive)[0])
    if count==0:
        if last!=0 or adaptive and step!=0:
            raise RuntimeError('zero foldbacks retained a nonzero duty/step')
        effective=requested
    else:
        if not 40<=last<requested:
            raise RuntimeError(f'invalid current-derived ceiling: requested={requested} count={count} last={last}')
        if adaptive and (not 10<=step<=50 or step%10):
            raise RuntimeError(f'invalid adaptive foldback step: {step}')
        if fixed and (requested-last)%step:
            raise RuntimeError(f'invalid fixed-step current ceiling: requested={requested} last={last} step={step}')
        effective=last
    return dict(count=count,last_duty=last,effective_duty=effective,
                step_tenths=step,step_policy='adaptive' if adaptive else 'fixed',release='none')

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--nominal-average',action='store_true',help='explicit uncalibrated nominal-current setting; target ACK required')
    ap.add_argument('--nominal-target-ma',type=int,choices=(800,1000,1500,2000,2500,3500,4000),default=800,help='require this physical target in the nominal-current ACK')
    ap.add_argument('--current-foldback',action='store_true',help='require and validate foreground5-percent foldback reporting')
    ap.add_argument('--lean-explore',action='store_true',help='retain lean exploration outcome, not legacy diagnostic qualification')
    ap.add_argument('--qualify-hold',action='store_true',help='require an uninterrupted protected lean hold at the requested live duty')
    ap.add_argument('--change69',action='store_true',help='query at7s, request69 at8s only after complete70 reply')
    ap.add_argument('--up-stop',action='store_true',help='70->69->73 with ACKs then host stop at11s')
    ap.add_argument('--step80',action='store_true',help='one guarded70->80 exploration; retain nondeadline stops')
    ap.add_argument('--duty50',action='store_true',help='require explicit firmware50-percent live envelope')
    ap.add_argument('--step-duty',type=int,choices=range(40,501),help='live target in tenths percent; above300 requires --duty50')
    ap.add_argument('--ramp-duty',type=int,choices=range(71,501),help='ACK-paced live ramp; above300 requires --duty50')
    ap.add_argument('--ramp-step',type=int,choices=range(1,51),default=50,help='live-ramp increment in tenths percent (default50 = 5 percent)')
    ap.add_argument('--ramp-period',type=float,default=.5,help='minimum seconds between acknowledged live-ramp steps (default0.5)')
    ap.add_argument('--ms',type=int,default=10000,help='finite powered window in milliseconds')
    ap.add_argument('--target-hold-ms',type=int,help='minimum MCU-timestamped dwell at ACKed target duty')
    ap.add_argument('--direct-start',action='store_true',help='firmware sine ramp100->200Hz, instead of host50->200Hz stepping')
    ap.add_argument('--normal-restart',action='store_true',help='inject tracking loss at2s; firmware must use normal startup, not flying reentry')
    ap.add_argument('--restart-duty',type=int,choices=range(71,501),help='before injected loss, request this live duty; above300 requires --duty50')
    ap.add_argument('--startup-ramp-start',type=float,default=3.2,help='host ramp start seconds after run50; default historical3.2')
    ap.add_argument('--startup-ramp-span',type=float,default=1.2,help='host ramp span seconds; finish before4.5s')
    ap.add_argument('--startup-duty',type=int,choices=range(40,101),default=62,help='sine catch/target duty in tenths percent')
    ap.add_argument('--catch-duty',type=int,choices=range(40,101),help='initial sine duty; defaults to startup duty, ramps to target in firmware')
    ap.add_argument('--drive-duty',type=int,choices=range(40,101),default=61,help='under-drive startup duty in tenths percent')
    ap.add_argument('--drive-phase',type=int,choices=(-30,0,30,60),default=60,help='existing one-shot sine-to-six-step phase shift, degrees')
    ap.add_argument('--bemf-duty',type=int,choices=range(40,301),default=70,help='initial BEMF duty, before live throttle steps, tenths percent; above100 requires timed startup')
    ap.add_argument('--fast-cycle-report',action='store_true',help='explicit report-only fast-cycle policy')
    ap.add_argument('--event100',action='store_true',help='explicit100us event floor')
    ap.add_argument('--dma-guard',action='store_true',help='explicit complete-scan IRQ guard publication')
    ap.add_argument('--carrier-hz',type=int,choices=(20000,24006,32000,40000),default=20000,
                    help='require the selected BEMF PWM carrier (default20000)')
    ap.add_argument('--fast-coast',action='store_true',help='explicit8kHz bridge-off coast diagnostic')
    ap.add_argument('--cycle400',action='store_true',help='explicit2500us running profile')
    ap.add_argument('--cycle450',action='store_true',help='explicit2223us running profile; no guard changes')
    ap.add_argument('--seed400',action='store_true',help='explicit shared834/5000/476 seed profile')
    ap.add_argument('--seed500',action='store_true',help='explicit shared667/4000/476 seed profile')
    ap.add_argument('--final-edge-prepare',action='store_true',help='explicit preparation build; live nonrecovery must report unused')
    args=ap.parse_args()
    if args.lean_explore and not args.nominal_average:ap.error('lean exploration requires explicit current setting')
    if args.qualify_hold and (not args.lean_explore or args.normal_restart):ap.error('qualify-hold requires lean exploration without injected restart')
    if args.qualify_hold and args.target_hold_ms is None:ap.error('qualify-hold requires --target-hold-ms; powered-window length is not target dwell')
    if args.target_hold_ms is not None and (not args.qualify_hold or not 1000<=args.target_hold_ms<=args.ms):ap.error('target-hold-ms requires qualify-hold and must fit within --ms')
    if args.current_foldback and not args.nominal_average:ap.error('current foldback requires explicit current setting')
    if sum([args.change69,args.up_stop,args.step80,args.step_duty is not None,args.ramp_duty is not None,args.restart_duty is not None])>1: ap.error('choose one experiment')
    if not 10000<=args.ms<=600000:ap.error('window must be10000..600000ms')
    if not .1<=args.ramp_period<=2.0:ap.error('ramp period must be0.1..2.0s')
    if args.fast_cycle_report and not args.cycle450:ap.error('fast-cycle-report requires --cycle450')
    if args.event100 and not args.fast_cycle_report:ap.error('event100 requires --fast-cycle-report')
    if args.cycle400 and args.cycle450:ap.error('choose one running profile')
    if args.seed400 and args.seed500:ap.error('choose one seed profile')
    # The firmware's *second* startup is autonomous regardless of whether the
    # host used the legacy 50->200 eHz ramp for the first startup. Keeping the
    # latter available avoids conflating first-start transfer reliability with
    # normal-restart duty restoration.
    if args.restart_duty is not None and not args.normal_restart:ap.error('restart-duty requires normal-restart')
    selected=max(v or 0 for v in (args.step_duty,args.ramp_duty,args.restart_duty))
    if selected>300 and not args.duty50:ap.error('live duty above300 requires --duty50')
    step_duty=80 if args.step80 else args.step_duty if args.step_duty is not None else args.ramp_duty
    with args.out.open('xb'):
        pass
    original=drv_capture.send_line
    armed=False
    state=0
    requested=70
    sent_at=0.0
    def poll(port,elapsed,data):
        nonlocal state,requested,sent_at
        if not (args.change69 or args.up_stop or step_duty is not None or args.restart_duty is not None) or b'TIMING energized_us=' in data:
            return
        if args.restart_duty is not None:
            if state==0 and elapsed>=5.0:
                drv_capture.send_live_line(port,'?');state=1
            elif state==1 and re.search(rf'D={args.bemf_duty:04X} I=[0-9A-F]{{4}} \r?\n'.encode(),data):
                requested=min(args.bemf_duty+50,args.restart_duty)
                drv_capture.send_live_line(port,f'du{requested}');state=2;sent_at=elapsed
            elif state==2 and requested<args.restart_duty and elapsed-sent_at>=0.35:
                if re.search(rf'D={requested:04X} I=[0-9A-F]{{4}} \r?\n'.encode(),data):
                    requested=min(requested+50,args.restart_duty)
                    drv_capture.send_live_line(port,f'du{requested}');sent_at=elapsed
            return
        if state==0 and elapsed>=7:
            drv_capture.send_live_line(port,'?');state=1
        elif state==1 and elapsed>=8 and re.search(rf'D={args.bemf_duty:04X} I=[0-9A-F]{{4}} \r?\n'.encode(),data):
            requested=min(max(100,args.bemf_duty),step_duty) if args.ramp_duty is not None else step_duty
            drv_capture.send_live_line(port,f'du{requested}' if requested is not None else 'du69');state=2;sent_at=elapsed
        elif args.ramp_duty is not None and state==2 and requested<step_duty and elapsed-sent_at>=args.ramp_period:
            if re.search(rf'D={requested:04X} I=[0-9A-F]{{4}} \r?\n'.encode(),data):
                requested=min(requested+args.ramp_step,step_duty)
                drv_capture.send_live_line(port,f'du{requested}');sent_at=elapsed
        elif args.up_stop and state==2 and elapsed>=9 and re.search(rb'D=0045 I=[0-9A-F]{4} \r?\n',data):
            drv_capture.send_live_line(port,'du73');state=3
        elif args.up_stop and state==3 and elapsed>=11 and re.search(rb'D=0049 I=[0-9A-F]{4} \r?\n',data):
            drv_capture.send_live_line(port,'off');state=4
    def send(port,command):
        nonlocal armed
        # Idle setup only. Preserve the fixture's complete motor-time schedule.
        if command=='cap1':
            if args.nominal_average:
                original(port,'avgnominal')
                avg=drv_capture.read_available(port,.15)
                with args.out.with_suffix('.average.txt').open('xb') as record:record.write(avg)
                nominal=re.search(rf'AVGNOMINAL accepted=1 raw_sum=(\d+) scans=(20|50|100) target_ma={args.nominal_target_ma} vdda_mv=(\d+) gain=10 shunt_mohm=7 calibrated=0 uncertainty_bounded=0'.encode(),avg)
                if not nominal or not 2700<=int(nominal[3])<=3600 or int(nominal[1])!=args.nominal_target_ma*7*10*4096*int(nominal[2])//(1000*int(nominal[3])):
                    raise RuntimeError(f'nominal current setting refused: {avg!r}')
            original(port,'live1')
            ack=drv_capture.read_available(port,.15)
            with args.out.with_suffix('.arm.txt').open('xb') as record:
                record.write(ack)
            duty_max=500 if args.duty50 else 300
            if f'LIVE armed=1 one_shot=1 duty_max={duty_max}'.encode() not in ack:
                raise RuntimeError(f'live arm refused: {ack!r}')
            armed=True
        original(port,command)
    drv_capture.send_line=send
    try:
        text=drv_capture.live_capture('COM41',115200,200 if args.direct_start else 50,args.startup_duty,False,None,
            raw_path=args.out,step_targets=() if args.direct_start else tuple(range(60,201,10)),catch_duty=args.catch_duty if args.catch_duty is not None else args.startup_duty,
        capture_stride=19,driven=True,drive_duty=args.drive_duty,drive_phase=args.drive_phase,
        step_start_s=args.startup_ramp_start,step_span_s=args.startup_ramp_span,
            drive_handoff=True,engage_ms=args.ms,core_trace=0,bemf_duty=args.bemf_duty,
            dropout=args.normal_restart and args.restart_duty is None,live_poll=poll,
            dropout_ms=5000 if args.restart_duty is not None else 2000,
            tracking_trip=args.restart_duty is not None)
    finally:
        drv_capture.send_line=original
    assert armed
    if args.lean_explore:
        result=lean_exploration_report(text)
        effective_restart=args.restart_duty
        foldback_count=0
        if args.current_foldback:
            requested=args.restart_duty if args.restart_duty is not None else step_duty
            if requested is None:raise RuntimeError('foldback validation needs a live duty request')
            result['current_foldback']=current_foldback_report(text,requested)
            foldback_count=result['current_foldback']['count']
            if effective_restart is not None:
                effective_restart=result['current_foldback']['effective_duty']
        if args.normal_restart:
            if result['lean_summary_seen']:
                result['normal_restart']=normal_restart_compact_report(
                    text,expected_resume=effective_restart,
                    expected_steps=(max(0,(effective_restart-args.bemf_duty+49)//50)
                        if effective_restart is not None and foldback_count==0 else None))
            else:
                result['normal_restart']=normal_restart_report(text,require_outcome=True,
                    expected_resume=effective_restart,foldback_after_restore=foldback_count>0)
            if args.restart_duty is not None:
                result['requested_duty']=args.restart_duty
                result['target_acknowledged']=bool(re.search(rf'D={args.restart_duty:04X} I=[0-9A-F]{{4}} \r?\n',text))
                if state!=2 or not result['target_acknowledged']:
                    raise RuntimeError('pre-loss restart duty was not acknowledged')
        if step_duty is not None:
            result['requested_duty']=step_duty
            result['target_acknowledged']=bool(re.search(rf'D={step_duty:04X} I=[0-9A-F]{{4}} \r?\n',text))
        if args.qualify_hold:
            if step_duty is None:raise RuntimeError('qualify-hold requires a duty target')
            result['hold']=lean_hold_report(text,step_duty,args.ms,args.target_hold_ms)
        print(result)
        return
    verify_seed_timing(text,True)
    from drv_fast_cycle import verify as verify_fast_cycle
    verify_fast_cycle(text,args.fast_cycle_report)
    from drv_dma_guard import verify as verify_dma_guard
    verify_dma_guard(text,args.dma_guard)
    from drv_fast_coast import verify as verify_fast_coast
    print('COASTCADENCE',verify_fast_coast(text,args.fast_coast))
    verify_running_profile(text,args.cycle400,args.cycle450,args.event100)
    from drv_seed_profile import verify as verify_seed_profile
    verify_seed_profile(text,args.seed400,seed500=args.seed500)
    from drv_final_preparation import verify as verify_final_preparation
    verify_final_preparation(text,args.final_edge_prepare,used=False)
    verify_mode(text,required=True,carrier_hz=args.carrier_hz)
    if args.up_stop or step_duty is not None:
        from drv_sustained_report import summarize
        from drv_driven_run import verify as acquisition
        from drv_driven_handoff import transfer_provenance
        drv_capture.parse_dump(text)
        transfer=transfer_provenance(text,acquisition(text,transfer=True))
        result=summarize(text)
        print(result)
        if step_duty is not None:
            if result['outputs_off_verified']!=1 or transfer['reported_power_reason']!=int(result['powered_reason']):
                raise RuntimeError('inconsistent stop/finaloff')
            if state!=2 or not re.search(rf'D={step_duty:04X} I=[0-9A-F]{{4}} \r?\n',text):
                raise RuntimeError('requested duty not acknowledged; not a successful live step')
            print(f'LIVE{step_duty} acknowledged; outcome above is exploratory, not qualification')
        elif (state!=4 or str(result['powered_reason'])!='9'
                or transfer['reported_power_reason']!=9 or result['outputs_off_verified']!=1
                or int(result['observed_us'])>=10000000):
            raise RuntimeError('live up/stop experiment did not complete requested sequence')
        else:
            print('LIVECHANGE 70->69->73 acknowledged, HostAbort and finaloff verified; mixed-duty run')
    else:
        print(verify(text,args.ms,dropout=False,reentry=False))
    if args.change69:
        if state!=2 or not re.search(r'D=0045 I=[0-9A-F]{4} \r?\n',text):
            raise RuntimeError('live69 acknowledgement missing; not a successful duty-change test')
        print('LIVECHANGE 70->69 acknowledged; no independent per-segment speed claim')


if __name__=='__main__':
    main()
