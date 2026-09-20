"""Offline sustained-run accounting. Completion of a window is not proof of lock.

Preserve all attempts, including pre-handoff failures. Verify raw ADC frames,
accepted-event windows and final output-off readback before classifying.
"""
import argparse
import csv
import math
import re
from pathlib import Path

from drv_capture import parse_dump, verify_off
from drv_accepted_events import decode_windows,decode_first_segment
from drv_event_timeline import decode as decode_timeline
from drv_adc_occupancy import decode as decode_adc_occupancy
from drv_adc_phase import decode as decode_adc_phase
from drv_current_sums import decode as decode_current_sums
from drv_cpu_meter import decode as decode_cpu_meter
from drv_cycle_fault import decode as decode_cycle_fault
from drv_cycle_fault import core_snapshot as decode_cycle_core
from drv_prestart_baseline import decode as decode_prestart_baseline
from drv_irq_trace import decode as decode_irq_trace


def fields(text, prefix):
    match=re.search(r'^'+re.escape(prefix)+r' ([^\r\n]+)',text,re.M)
    return dict(re.findall(r'(\w+)=(\d+)',match[1])) if match else {}


def summarize(text):
    text=text.replace('\r','')
    from drv_role_check import verify_mode
    verify_mode(text)
    decode_adc_phase(text)
    decode_cycle_fault(text)
    decode_cycle_core(text)
    decode_prestart_baseline(text)
    if re.search(r'^(?:IRQWINDOW|COMPMODE|COMPREAD) ',text,re.M):
        decode_irq_trace(text)
    if re.search(r'^(?:CPUMETER|CPU85|CPUUNION|CU85) ',text,re.M):
        decode_cpu_meter(text)
    if re.search(r'^(?:ADCSTATS|S85) ',text,re.M):
        decode_current_sums(text)
    _,_,_,_,_,startup_reason=parse_dump(text) # includes compact-frame CRC checks
    # Optional newer evidence must validate too; legacy captures remain valid.
    launch_coverage=None
    if re.search(r'^(?:ADCLAUNCH|AL85) ',text,re.M):
        launch_coverage=decode_adc_occupancy(text)
    # A preflight off readback is NOT evidence of post-run safing. Require
    # the readback after the final capture terminator, even in truncated logs.
    ended=text.rfind('COAST END')
    if ended<0: raise ValueError('missing final capture terminator')
    verify_off(text[ended+len('COAST END'):].encode())
    fly=fields(text,'FLY'); power=fields(text,'POWERPATH')
    profile=fields(text,'RUNLIMIT')
    if profile and profile not in (
        dict(cycle_min_us='4000',event_min_us='333',cycle_max_us='6000',event_max_us='1000',experimental='0'),
        dict(cycle_min_us='3333',event_min_us='277',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='3226',event_min_us='268',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='3125',event_min_us='260',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='3031',event_min_us='252',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2986',event_min_us='248',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2942',event_min_us='245',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2899',event_min_us='241',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2858',event_min_us='238',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2778',event_min_us='238',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2500',event_min_us='238',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2223',event_min_us='238',cycle_max_us='6000',event_max_us='1000',experimental='1'),
        dict(cycle_min_us='2223',event_min_us='100',cycle_max_us='6000',event_max_us='1000',experimental='1')):
        raise ValueError('unknown running guard profile; review before classifying')
    end=fields(text,'COASTREF'); quality=fields(text,'ACCEPTQUALITY')
    adc=fields(text,'POWERFEEDBACK'); com=fields(text,'POWERCOMMITS')
    injection=fields(text,'DROPOUT applied=1')
    reacq=fields(text,'RECOVERYACQ')
    cycle=fields(text,'RECOVERYCYCLE')
    faster_seed=(profile.get('experimental')=='1' and cycle.get('cycle_min_ticks')=='6666'
                 and cycle.get('individual_min_ticks')=='554')
    cycle_min=6666 if faster_seed else 8000
    individual_min=554 if faster_seed else 666
    seed_min=1111 if faster_seed else 1333
    if profile.get('cycle_min_us')=='3226':
        cycle_min,individual_min,seed_min=6452,536,1075
    if profile.get('cycle_min_us')=='3031':
        cycle_min,individual_min,seed_min=6062,504,1010
    elif profile.get('cycle_min_us')=='2986':
        cycle_min,individual_min,seed_min=5972,496,995
    elif profile.get('cycle_min_us')=='2942':
        cycle_min,individual_min,seed_min=5884,490,980
    elif profile.get('cycle_min_us')=='2899':
        cycle_min,individual_min,seed_min=5798,482,966
    elif profile.get('cycle_min_us') in ('2858','2778','2500','2223'):
        cycle_min,individual_min,seed_min=5716,476,952
    elif profile.get('cycle_min_us')=='3125':
        cycle_min,individual_min,seed_min=6250,520,1041
    cycle_required=bool(fields(text,'FLYCYCLE') or cycle)
    from drv_seed_profile import profile as seed_profile
    selected_seed=seed_profile(text)
    if selected_seed:cycle_min,individual_min,seed_min=selected_seed
    cycle_ok=(cycle.get('checked')=='7' and cycle.get('rejected_ticks')=='0'
              and cycle.get('cycle_min_ticks')==str(cycle_min) and cycle.get('cycle_max_ticks')=='12000'
              and cycle.get('individual_min_ticks')==str(individual_min)
              and cycle_min<=int(cycle.get('min_ticks','0'))<=int(cycle.get('max_ticks','0'))<=12000)
    reentry_match=re.search(r'^REENTRY result=(\d+) ',text,re.M)
    reserve_lines=re.findall(r'^REENTRYRESERVE (.*)$',text,re.M)
    reserve=200 # Original captures had an implicit200us reserve.
    if reserve_lines:
        match=re.fullmatch(r'us=(200|300) included_in_original_deadline=1',reserve_lines[0])
        if len(reserve_lines)!=1 or not match or not reentry_match:
            raise ValueError('invalid recovery reserve provenance')
        reserve=int(match[1])
    reentry=fields(text,'REENTRY result='+reentry_match[1]) if reentry_match else {}
    stack_reports=[tuple(map(int,m)) for m in re.findall(
        r'^(?:> )?STACK span=(\d+) painted=(\d+) untouched=(\d+) ',text,re.M)]
    stack_ok=bool(stack_reports) and all(span>=4096 and 512<=untouched<=painted<=span
                                        for span,painted,untouched in stack_reports)
    row=dict(startup_reason=startup_reason, acquisition_result=fly.get('result',''),
             run_cycle_min_us=profile.get('cycle_min_us',''),
             run_event_min_us=profile.get('event_min_us',''),
             experimental_running_guard=profile.get('experimental',''),
             adc_launch_records_verified=int(launch_coverage is not None),
             acquisition_deferred_polls=fields(text,'FLYWAIT').get('polls',''),
             seed_ticks=fly.get('interval_ticks',''), seed_sector=fly.get('step',''),
             powered_reason=power.get('reason',''), requested_us=end.get('max_us',''),
             observed_us=quality.get('end_us',''), commutations=com.get('applied',''),
             accepted_events=quality.get('events',''), peak_abs_raw=adc.get('peak_abs_raw',''),
             bus_min_mv=adc.get('bus_min_mv',''), outcome='startup_stopped',
             cycle_mean_us='',cycle_sigma_us='',accepted_rate_ehz='',omitted_events='',
             post_stop_tail_records='',outputs_off_verified=1,
             dropout_applied_us=injection.get('at_us',''),dropout_stop_verified=0,
             dropout_to_disabled_observation_us='',
             recovery_acquisition_result=reacq.get('result',''),
             recovery_acquisition_us=reacq.get('elapsed_us',''),
             recovery_seed_ticks=reacq.get('interval_ticks',''),
             recovery_cycles_verified=int(cycle_ok) if cycle_required else '',
             recovery_passive_seed_verified=0,
             reentry_result=reentry_match[1] if reentry_match else '',reentry_verified=0,
             stack_untouched_bytes=min((s[2] for s in stack_reports),default=''),
             original_end_elapsed_us=reentry.get('original_end_elapsed_us',''),
             final_elapsed_us=reentry.get('final_elapsed_us',''))
    row['timeline_verified']=''
    if 'TIMELINE label=' in text:
        decode_timeline(text,'ET85')
        if 'FIRSTSEG fault=' in text: decode_timeline(text,'FT85')
        row['timeline_verified']=1
    if fly:
        row['outcome']='acquisition_refused' if fly.get('result')!='1' else 'handoff_not_started'
    if 'ACCEPTLOG ' in text:
        windows=decode_windows(text)
        row['omitted_events']=windows['skipped']
        if quality and windows['total']!=int(quality['events']):
            raise ValueError('accepted total disagrees with whole-stream statistics')
        if quality:
            row['post_stop_tail_records']=sum(e['us']>int(quality['end_us'])
                for e in windows['prefix']+windows['tail'])
    if 'reason' in power:
        row['outcome']='powered_stopped'
        normal_stop=(power['reason']=='0' and end.get('stop')=='1') or (
            power['reason']=='2' and end.get('stop') in ('1','7')
            and int(power.get('stop_us','0'))>=int(end.get('max_us','1')))
        if (normal_stop and power.get('disabled')=='1' and power.get('active')=='0'
                and quality and int(quality['end_us'])>=int(end['max_us'])
                and row['post_stop_tail_records']==0
                and int(quality.get('last_us','0'))<=int(quality['end_us'])
                and quality.get('order_bad')=='0' and end.get('desync')=='0'
                and end.get('polling')=='0' and end.get('sense_mux_only')=='0'):
            row['outcome']='powered_window_complete'
    if injection and quality:
        # Same observer clock for injection, last event and post-stop end.
        # end_us is conservative disabled observation, not exact gate-off edge.
        at=int(injection['at_us']); last=int(quality['last_us']); done=int(quality['end_us'])
        row['dropout_to_disabled_observation_us']=done-at
        row['dropout_stop_verified']=int(
            power.get('reason')=='8' and power.get('disabled')=='1' and power.get('active')=='0'
            and injection.get('automatic_restart')=='0' and int(injection.get('scheduled_us','0'))==2000000
            and 2000000<=at<2001000 and last<=at<=done and 1000<done-last<=1200
            and quality.get('order_bad')=='0' and row['post_stop_tail_records']==0)
    if reacq:
        row['recovery_passive_seed_verified']=int(
            row['dropout_stop_verified']==1 and reacq.get('result')=='1'
            and reacq.get('disabled')=='1' and reacq.get('gate_authority')=='0'
            and reacq.get('intervals')=='12' and 1<=int(reacq.get('step','0'))<=6
            and seed_min<=int(reacq.get('interval_ticks','0'))<=2000
            and 0<int(reacq.get('elapsed_us','0'))<20000
            and 0<int(reacq.get('max_gap_ticks','0'))<=200)
    if reentry_match and reentry_match[1]=='7':
        first=decode_first_segment(text)
        original=int(reentry['original_end_elapsed_us'])
        resumed=int(reentry['resume_elapsed_us']); remaining=int(reentry['remaining_us'])
        final=int(reentry['final_elapsed_us'])
        row['reentry_verified']=int(
            row['outcome']=='powered_window_complete' and stack_ok and first['header']['fault']==8
            and first['total']>0 and first['header'].get('commits',0)>0
            and reacq.get('result')=='1' and reacq.get('intervals')=='12'
            and (not cycle_required or cycle_ok)
            and 1<=int(reacq.get('step','0'))<=6
            and 0<int(reacq.get('elapsed_us','0'))<20000
            and 0<int(reacq.get('max_gap_ticks','0'))<=200
            and reacq.get('gate_authority')=='0' and seed_min<=int(reacq.get('interval_ticks','0'))<=2000
            and reentry.get('attempts_max')=='1' and 2000000<=int(reentry['first_injection_us'])<2001000
            and resumed+remaining+reserve==original and resumed<final<=original
            and remaining==int(row['requested_us']) and int(row['accepted_events'])>=12)
    moments=re.search(r'^ACCEPTMOMENTS kind=cycle ([^\r\n]+)',text,re.M)
    if moments:
        m=dict((k,int(v)) for k,v in re.findall(r'(\w+)=(\d+)',moments[1]))
        if m['n']:
            mean=m['sum_us']/m['n']
            row['cycle_mean_us']=mean
            row['cycle_sigma_us']=math.sqrt(max(0,m['squares_us']/m['n']-mean*mean))
            row['accepted_rate_ehz']=1_000_000/mean if mean else ''
        if m['excluded'] and row['outcome']=='powered_window_complete':
            raise ValueError('completed window has excluded cycle measurements')
    from drv_fast_cycle import decode as decode_fast_cycle
    fast=decode_fast_cycle(text)
    row['fast_cycle_report_only']=int(fast is not None)
    row['fast_cycle_count']=fast['count'] if fast else ''
    row['fast_cycle_min_us']=fast['min_us'] if fast else ''
    return row


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--prefix',required=True,help='capture filename prefix; every matching attempt included')
    p.add_argument('--out',type=Path,required=True,help='new CSV report path; refuses overwrite')
    args=p.parse_args()
    root=Path(__file__).resolve().parents[1]/'captures'
    captures=sorted(root.glob(args.prefix+'*.txt'))
    if not captures: p.error('no captures matched')
    rows=[dict(capture=f.name,**summarize(f.read_text())) for f in captures]
    with args.out.open('x',newline='') as output:
        writer=csv.DictWriter(output,fieldnames=list(rows[0]))
        writer.writeheader();writer.writerows(rows)
    complete=sum(r['outcome']=='powered_window_complete' for r in rows)
    print(f'{complete}/{len(rows)} complete powered windows; not a lock/parity certificate')
    for r in rows: print(r['capture'],r['outcome'],r['accepted_rate_ehz'])


if __name__=='__main__': main()
