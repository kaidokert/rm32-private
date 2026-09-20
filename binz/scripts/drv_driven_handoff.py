"""Explicit driven-to-BEMF transfer campaign; never an observer-only pass."""
import argparse
import json
import re
from pathlib import Path
from drv_capture import live_capture
from drv_driven_run import records,verify as verify_acquisition
from drv_sustained_report import summarize
from drv_accepted_events import decode_first_segment,decode_windows


def first_arm(text):
    header='DRIVENFIRST fields=interval_ticks,edge_age_ticks,remaining_arr,arm_us,armed,first_elapsed_us,requested_us u32_le_pairs=1 before_reentry=1'
    words=records(text,'DFA85',14)
    if text.count(header)!=1 or len(words)!=1: raise ValueError('missing initial-arm archive')
    values=[words[0][i]|words[0][i+1]<<16 for i in range(0,14,2)]
    if values[4]!=1: raise ValueError('initial handoff never armed')
    return values


def transfer_provenance(text,acquisition,*,reentry=False):
    """Check clock/seed identity separately from powered-window completion."""
    text=text.replace('\r','')
    yields=re.findall(r'^DRIVENYIELD (.*)$',text,re.M)
    if yields:
        if len(yields)!=1 or not re.fullmatch(r'abandoned=(0|1) completed_channels=[0-5] partial_feedback_published=0',yields[0]):
            raise ValueError('invalid partial-scan yield provenance')
        if yields[0].startswith('abandoned=0') and 'completed_channels=0 ' not in yields[0]:
            raise ValueError('unabandoned scan has yield count')
    headers=re.findall(r'^DRIVEX result=(\d+) power_reason=(\d+) fresh_transfer=1$',text,re.M)
    fields='DX85FIELDS step,edge_half_us,mean_half_us,release_us,foreground_us,feedback_age_us,foreground_entered'
    rows=records(text,'DX85',7);seeds=records(text,'DS85',10)
    if len(headers)!=1 or headers[0][0]!='1' or text.count(fields)!=1 or len(rows)!=1 or len(seeds)!=1:
        raise ValueError('missing successful transfer entry/provenance')
    step,edge,mean,release,foreground,age,entered=rows[0]
    ready,ss,lo,hi,average,intervals,cycles,_,_,fault=seeds[0]
    if (not ready or fault or intervals!=12 or cycles!=7 or hi or
            (step,edge,mean)!=(ss,lo,average) or not 1333<=mean<=2000):
        raise ValueError('transfer seed identity/qualification failed')
    if not (entered==1 and edge<=2*release and acquisition['start_us']+1100<=release
            <=acquisition['stop_us']<=foreground<release+1000 and 0<age<=1000):
        raise ValueError('transfer release/foreground/feedback timing failed')
    # LAST_FEEDBACK must retain the original completed ADC acquisition stamp.
    adc=records(text,'DA85',7)
    if not adc or not -2<=foreground-age-adc[-1][0]<=0:
        raise ValueError('transfer refreshed or mismatched ADC acquisition timestamp')
    core=re.findall(r'^CORESEED assumed=0 source=measured_flying interval_ticks=(\d+) edge_age_ticks=(\d+) remaining_arr=(\d+) arm_us=(\d+) bootstrap_com=1 synthetic_accept=0$',text,re.M)
    if len(core)!=1 or re.findall(r'^CORESEED armed=(\d+) refusal_stop_code=8$',text,re.M)!=['1']:
        raise ValueError('missing fresh measured core arm')
    ci,edge_age,remaining,arm=first_arm(text)[:4] if reentry else map(int,core[0])
    wait=ci//2-((ci*16)>>6)
    if ci!=mean or edge_age<2*foreground-edge or edge_age+remaining!=wait or remaining<64 or arm>16:
        raise ValueError('stale seed or insufficient actual arm margin')
    return dict(seed_step=step,seed_ticks=mean,release_to_foreground_us=foreground-release,
                feedback_age_us=age,remaining_half_us=remaining,arm_us=arm,
                reported_power_reason=int(headers[0][1]))


def reject_pre_handoff_fault(text):
    if (re.search(r'^DRIVEOBS reason=21 ',text,re.M)
            and re.search(r'^DRIVENIRQ calls=65 accepts=0 .*rate_peak=65 rate_limit=64 ',text,re.M)):
        from drv_capture import parse_dump,verify_off
        parse_dump(text)
        marker=text.rfind('FINALOFF\n')
        if marker<0:raise ValueError('qualification rate refusal lacks finaloff')
        verify_off(text[marker:].encode())
        raise ValueError('initial qualification IRQ-rate refusal before BEMF handoff; CRC/finaloff verified')
    # A stop before driven qualification has no later build-provenance rows.
    # Identify the actual retained startup stop before diagnosing missing rows.
    if ('current ADC rail/peak -> gates + en OFF; coast capture' not in text
            or re.search(r'^DRIVENIRQ ',text,re.M)):
        return
    from drv_capture import parse_dump,verify_off
    if parse_dump(text)[5]!=4:
        raise ValueError('inconsistent pre-handoff current stop capture')
    marker=text.rfind('FINALOFF\n')
    if marker<0:raise ValueError('pre-handoff current stop lacks finaloff')
    verify_off(text[marker:].encode())
    raise ValueError('startup current ADC rail/peak stop before driven qualification; CRC/finaloff verified')

def startup_plan(direct):
    return (200,()) if direct else (50,tuple(range(60,201,10)))

def verify_direct_startup(text):
    from drv_capture import parse_dump
    clean=text.replace('\r','')
    if not re.search(r'^(?:> )?run200$',clean,re.M) or 'ramp=2000ms target=200Hz/6.2%' not in clean:
        raise ValueError('direct startup command/configuration missing')
    if re.search(r'^(?:> )?ehz\d+',clean,re.M):
        raise ValueError('unexpected host frequency steps during direct startup')
    rows=parse_dump(text)[0]
    ramp=[r['freq_chz'] for r in rows if r['stage']==3]
    if len(ramp)<2 or any(not 10000<=v<=20000 for v in ramp) or ramp!=sorted(ramp):
        raise ValueError('direct startup ramp samples inconsistent')
    return dict(first_chz=ramp[0],last_chz=ramp[-1],records=len(ramp),monotonic=True)

def verify(text,expected_ms=None,*,dropout=False,reentry=False):
    if reentry and re.search(r'^CORESEED armed=0 refusal_stop_code=8$',text,re.M):
        # Report a fresh-arm refusal as such, never as missing telemetry or a
        # recovery pass. Check retained evidence before describing its margin.
        from drv_capture import parse_dump,verify_off
        parse_dump(text)
        marker=text.rfind('FINALOFF\n')
        if marker<0:raise ValueError('recovery arm refusal lacks finaloff evidence')
        verify_off(text[marker:].encode())
        first=decode_first_segment(text)
        rows=re.findall(r'^CORESEED assumed=0 source=measured_flying interval_ticks=(\d+) edge_age_ticks=(\d+) remaining_arr=(\d+) arm_us=(\d+) bootstrap_com=1 synthetic_accept=0$',text,re.M)
        acquired=re.findall(r'^RECOVERYACQ result=1 step=[1-6] interval_ticks=(\d+) elapsed_us=\d+ intervals=12 max_gap_ticks=\d+ disabled=0 gate_authority=0$',text,re.M)
        if len(rows)!=1 or len(acquired)!=1 or not first or first['header']['fault']!=8:
            raise ValueError('invalid recovery arm refusal evidence')
        ci,age,arr,arm=map(int,rows[0])
        if ci!=int(acquired[0]):raise ValueError('recovery arm refusal seed mismatch')
        remaining=ci//2-((ci*16)>>6)-age
        if arr==0xffffffff and arm==0 and remaining<64:
            raise ValueError(f'recovery fresh-arm margin refused: remaining_ticks={remaining} <64, '
                             f'edge_age_ticks={age}; CRC/first archive/finaloff verified')
        raise ValueError('recovery arm refused; not a qualified recovery; CRC/first archive/finaloff verified')
    if reentry and re.search(r'^REENTRY result=5 ',text,re.M):
        # Acquisition can refuse before installing the recovery guard. Its
        # staged=0 and empty second-segment stats are expected, not the cause.
        from drv_capture import parse_dump,verify_off
        parse_dump(text)
        marker=text.rfind('FINALOFF\n')
        if marker<0:raise ValueError('recovery refusal lacks finaloff evidence')
        verify_off(text[marker:].encode())
        first=decode_first_segment(text)
        rows=re.findall(r'^RECOVERYACQ result=(\d+) step=(\d+) interval_ticks=(\d+) elapsed_us=(\d+) intervals=(\d+) max_gap_ticks=(\d+) disabled=1 gate_authority=0$',text,re.M)
        if len(rows)!=1 or not first or first['header']['fault']!=8:
            raise ValueError('invalid recovery acquisition refusal evidence')
        result,step,interval,elapsed,count,gap=map(int,rows[0])
        if result==1:raise ValueError('successful acquisition contradicts reentry result5')
        raise ValueError(f'recovery acquisition refused: result={result}, elapsed_us={elapsed}, '
                         f'intervals={count}; no second powered segment; CRC/first archive/finaloff verified')
    if reentry and not dropout: raise ValueError('reentry requires explicit dropout campaign')
    early=re.findall(r'^REENTRY result=1 first_injection_us=0 resume_elapsed_us=0 remaining_us=0 ',text,re.M)
    if reentry and len(early)==1:
        # An initial guard trip can precede both injection and recovery staging.
        # Validate the retained initial segment, but NEVER accept this as a run
        # or recovery pass or demand a recovery-only arm archive it cannot have.
        acquisition=verify_acquisition(text,transfer=True)
        transfer_provenance(text,acquisition,reentry=False)
        powered=summarize(text)
        raise ValueError('initial segment stopped before dropout; recovery not exercised; '
                         f"reason={powered['powered_reason']}, outcome={powered['outcome']}, "
                         f"outputs_off_verified={powered['outputs_off_verified']}")
    staged=re.findall(r'^REENTRYSTATS ([^\r\n]+)',text,re.M)
    if staged and staged!=[f'used={int(reentry)} preparation_before_acquisition=1 gate_authority=0']:
        raise ValueError('reentry statistics staging provenance failed')
    acquisition=verify_acquisition(text,transfer=True)
    transfer=transfer_provenance(text,acquisition,reentry=reentry)
    # These summaries are singleton records, not first-match alternatives.
    for label in ['POWERPATH','COASTREF','ACCEPTQUALITY','POWERFEEDBACK','POWERCOMMITS','RUNLIMIT']:
        if len(re.findall(r'^'+label+r' ',text,re.M))!=1:
            raise ValueError('missing or duplicated '+label)
    powered=summarize(text)
    outcome_ok=powered['reentry_verified']==1 if reentry else (powered['dropout_stop_verified']==1 if dropout else powered['outcome']=='powered_window_complete')
    if (not outcome_ok or powered['timeline_verified']!=1
            or int(powered['accepted_events'])<12 or int(powered['commutations'])<12
            or int(powered['bus_min_mv'])<8400 or int(powered['peak_abs_raw'])>1200
            or int(powered['powered_reason'])!=transfer['reported_power_reason']):
        raise ValueError('transfer entered but powered tracking window did not complete: '+str(powered))
    requested=int(powered['requested_us'])
    if reentry:
        initial=first_arm(text);requested=initial[6]
        final=re.search(r'^CORESEED assumed=0 source=measured_flying interval_ticks=(\d+) edge_age_ticks=(\d+) remaining_arr=(\d+) arm_us=(\d+)',text,re.M)
        ci,age,remaining,arm=map(int,final.groups())
        if ci!=int(powered['recovery_seed_ticks']) or age+remaining!=ci//2-((ci*16)>>6) or remaining<64 or arm>16:
            raise ValueError('recovery seed/actual arm disagrees with fresh reacquisition')
        first=decode_first_segment(text)
        last=max(e['us'] for e in first['prefix']+first['tail'])
        if (initial[5]+requested!=int(powered['original_end_elapsed_us'])
                or requested<=2000000 or first['header']['events']<12
                or first['header']['commits']<12 or first['header']['peak_raw']>1200
                or first['header']['bus_min']<8400 or not 1000<first['header']['end_us']-last<=1200):
            raise ValueError('initial segment deadline/electrical/tracking-stop archive failed')
    if not 20000<=requested<=600000000 or (expected_ms is not None and requested!=expected_ms*1000):
        raise ValueError('wrong powered test duration')
    if dropout and not reentry:
        first=decode_first_segment(text);primary=decode_windows(text)
        if (requested<=2000000 or first['header']['fault']!=8 or
                first['header']['commits']!=int(powered['commutations']) or
                any(first[k]!=primary[k] for k in ['total','prefix','tail','skipped']) or
                re.search(r'^REENTRY result=',text,re.M)):
            raise ValueError('dropout must freeze the original stream without powered restart')
    return dict(acquisition=acquisition,transfer=transfer,powered=powered,
                powered_handoff_window_verified=not dropout,tracking_loss_stop_verified=dropout,
                powered_recovery_verified=reentry,
                independent_bemf_lock_proven=False)


def verify_core_trace(text, requested):
    if requested is None:
        return
    markers=re.findall(r'^CORETRACE enabled=([01]) per_read_bookkeeping=([01])$',text,re.M)
    if requested not in (0,1) or markers!=[(str(requested),str(requested))]:
        raise ValueError('applied comparator trace mode differs from requested experiment')


def verify_prestart_dma(text, required=False):
    from drv_prestart_baseline import decode
    baseline=decode(text)
    present=bool(baseline and baseline['acquisition']['dma'])
    if required!=present:raise ValueError('prestart DMA requires matching explicit fixture selection')


def verify_cycle450(text, required=False, event100=False):
    if event100 and not required:raise ValueError('event100 requires cycle450')
    rows=re.findall(r'^RUNLIMIT ([^\r\n]+)',text,re.M)
    candidate=f'cycle_min_us=2223 event_min_us={100 if event100 else 238} cycle_max_us=6000 event_max_us=1000 experimental=1'
    if required and rows!=[candidate]:raise ValueError('cycle450 profile missing/mismatched')
    if not required and any('cycle_min_us=2223' in row for row in rows):
        raise ValueError('cycle450 requires explicit fixture selection')

def verify_cycle400(text, required=False):
    rows=re.findall(r'^RUNLIMIT ([^\r\n]+)',text,re.M)
    candidate='cycle_min_us=2500 event_min_us=238 cycle_max_us=6000 event_max_us=1000 experimental=1'
    if required and rows!=[candidate]:raise ValueError('cycle400 profile missing/mismatched')
    if not required and any('cycle_min_us=2500' in row for row in rows):
        raise ValueError('cycle400 requires explicit fixture selection')

def verify_cycle360(text, required=False):
    rows=re.findall(r'^RUNLIMIT ([^\r\n]+)',text,re.M)
    candidate='cycle_min_us=2778 event_min_us=238 cycle_max_us=6000 event_max_us=1000 experimental=1'
    if required and rows!=[candidate]:raise ValueError('cycle360 profile missing/mismatched')
    if not required and any('cycle_min_us=2778' in row for row in rows):
        raise ValueError('cycle360 requires explicit fixture selection')


def verify_com_keep_running(text, required=False):
    rows=re.findall(r'^COMARM(?: ([^\r\n]*))?$',text,re.M)
    expected=['running_counter=1 stale_pending_cleared=1 stopped_start_retained=1 experimental=1']
    if rows != (expected if required else []):
        raise ValueError('COM arm experiment provenance mismatch')


def verify_seed_timing(text, required=False):
    rows=re.findall(r'^SEEDTIMING ([^\r\n]+)',text,re.M)
    if not required:
        if rows: raise ValueError('timing reanchor requires explicit fixture selection')
    elif len(rows)!=1 or not re.fullmatch(r'discarded_fault=(0|3) max_restarts=1 fresh_intervals=12 original_deadline=1 experimental=1',rows[0]):
        raise ValueError('missing/invalid timing reanchor provenance')
    if required and re.findall(r'^DRIVENIRQBUDGET ([^\r\n]+)',text,re.M)!=[
            'limit_us=50 overruns=0 immediate_stop=1 before_handoff=1 excludes_release=1']:
        raise ValueError('missing/failed live acquisition IRQ budget')


def verify_read_call(text, required=False):
    rows=re.findall(r'^COMPREADCALL ([^\r\n]+)',text,re.M)
    expected=['noninline=1 live_read=1 read_count_unchanged=1 timing_equivalence_proven=0'] if required else []
    if rows!=expected:
        raise ValueError('comparator call experiment expectation mismatch')

def verify_peer_priority(text,required=False,*,dma_peer=False,prepared_recovery=False):
    markers=re.findall(r'^COREPRIORITY ([^\r\n]+)',text,re.M)
    if markers or required or dma_peer or prepared_recovery:
        if prepared_recovery:
            from drv_prepared_handoff import verify as verify_prepared
            verify_prepared(text,True)
            if len(re.findall(r'^CORESEED armed=1 refusal_stop_code=8\r?$',text,re.M))!=1:
                raise ValueError('prepared priority requires successful measured arm')
        if markers!=[f'comp=64 com={0 if prepared_recovery else 64} guard=0 dma={64 if dma_peer else 0}']:
            raise ValueError('peer-priority/independent guard priority mismatch')

def verify_hysteresis(text,required=False):
    rows=re.findall(r'^COMPHYST ([^\r\n]+)',text,re.M)
    if rows or required:
        if rows!=['code=1 startup_and_bemf=1 raw_reads_unchanged=1']:
            raise ValueError('low comparator hysteresis readback mismatch')

def verify_static_comp(text,required=False):
    rows=re.findall(r'^COMPSTATIC(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['real_inverted_traceoff=1 live_reads=1 reference_core=1 diagnostic_fallback=1']:
            raise ValueError('static comparator mode provenance mismatch')

def verify_guard_codegen(text,required=False):
    rows=re.findall(r'^GUARDCODE(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['inline_constructor=1 admission_unchanged=1 prestaged=0']:
            raise ValueError('guard codegen provenance mismatch')

def verify_guard_install(text,required=False):
    rows=re.findall(r'^GUARDINSTALL(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['admission_token=1 fresh_checks=1 in_place=1 prestaged=0']:
            raise ValueError('guard installation provenance mismatch')


def verify_seed_math(text,required=False):
    rows=re.findall(r'^SEEDMATH(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['div12_bound=24000 exact=1 fallback=1 qualification_unchanged=1']:
            raise ValueError('seed arithmetic provenance mismatch')


def verify_reentry_clear(text,required=False):
    rows=re.findall(r'^REENTRYCLEAR(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['acquisition_clear=1 duplicate_omitted=1 live_checks=1 initial_unchanged=1']:
            raise ValueError('reentry clear provenance mismatch')


def verify_carrier_math(text,required=False):
    rows=re.findall(r'^CARRIERMATH(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['compare_at_prepare=1 steady_divide=0 exact_compare=1 duty_change_refused=1']:
            raise ValueError('carrier arithmetic provenance mismatch')


def verify_bin_math(text,required=False):
    rows=re.findall(r'^BINMATH(?: ([^\r\n]*))?$',text,re.M)
    if rows or required:
        if rows!=['adc_phase_divide=0 timeline_index_divide=0 exact=1 timestamps_unchanged=1']:
            raise ValueError('bin arithmetic provenance mismatch')


def verify_quiet_irq_stamp(text):
    rows=re.findall(r'^IRQSTAMP ([^\r\n]+)',text,re.M)
    if rows!=['omitted=1 report_only=1 accepted_clock_unchanged=1 safety_clocks_unchanged=1']:
        raise ValueError('report-only IRQ timestamp provenance missing/invalid')
    verify_core_trace(text,0)


def verify_reentry_carrier(text):
    rows=re.findall(r'^REENTRYCARRIER ([^\r\n]+)',text,re.M)
    if rows!=['prepared_before_edge=1 live_period_check=1 output_authority_retained=0 failure_reset_unchanged=1']:
        raise ValueError('recovery carrier provenance missing/invalid')


def main():
    from drv_role_check import verify_mode
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path);ap.add_argument('--infile',type=Path)
    ap.add_argument('--startup-direct',action='store_true',help='host-only run200 monotonic startup; unchanged duty and firmware')
    ap.add_argument('--ms',type=int,default=20)
    ap.add_argument('--dropout',action='store_true',help='suppress COMP at2s; verify stop/archive,NO powered restart')
    ap.add_argument('--reentry',action='store_true',help='with dropout: one bounded powered recovery,original deadline')
    ap.add_argument('--drive-duty',type=int,choices=range(40,63),default=46)
    ap.add_argument('--bemf-duty',type=int,choices=range(40,101),help='independent fixed BEMF segment duty in tenths percent; startup unchanged')
    ap.add_argument('--phase-shift',type=int,choices=[-30,0,30,60],default=60)
    ap.add_argument('--core-trace',type=int,choices=[0,1],help='explicit existing per-read instrumentation A/B; cleanup leaves tracing off')
    ap.add_argument('--pwm-roles',action='store_true',help='require BEMF-only free-running carrier build provenance')
    ap.add_argument('--carrier-hz',type=int,choices=[10000,20000,24006,32000,40000],help='require exact carrier provenance (24k nominal is64000000/2666)')
    ap.add_argument('--inline-comp',action='store_true',help='require inlined comparator adapter experiment')
    ap.add_argument('--comp-read-call',action='store_true',help='require noninline comparator-read cadence experiment')
    ap.add_argument('--com-keep-running',action='store_true',help='require running-counter COM arm experiment')
    ap.add_argument('--cycle360',action='store_true',help='require2778us cycle floor with unchanged238us event and350seed guards')
    ap.add_argument('--cycle400',action='store_true',help='require2500us cycle floor, unchanged238us event and350seed guards')
    ap.add_argument('--cycle450',action='store_true',help='require2223us running cycle floor; seed profile selected separately')
    ap.add_argument('--seed400',action='store_true',help='explicit shared834/5000/476 measured-seed policy')
    ap.add_argument('--seed450',action='store_true',help='explicit shared741/4445/476 recovery seed policy')
    ap.add_argument('--seed500',action='store_true',help='explicit shared667/4000/476 recovery seed policy')
    ap.add_argument('--prestart-dma',action='store_true',help='require same-DMA pre-drive baseline provenance, not calibrated current')
    ap.add_argument('--fast-cycle-report',action='store_true',help='explicit operator-approved report-only fast-cycle floor')
    ap.add_argument('--event100',action='store_true',help='explicit100us event floor; order and1ms stale limit unchanged')
    ap.add_argument('--dma-guard',action='store_true',help='explicit complete-scan IRQ guard publication')
    ap.add_argument('--final-edge-prepare',action='store_true',help='require explicit pre-final-edge preparation provenance')
    ap.add_argument('--seed-timing-reanchor',action='store_true',help='require explicit one-restart long-interval acquisition experiment')
    ap.add_argument('--comp-paths',action='store_true',help='require dispatched comparator-path counts and full architecture accounting')
    ap.add_argument('--single-core-atomics',action='store_true',help='require experimental single-core atomic backend')
    ap.add_argument('--peer-priority',action='store_true',help='require COMP/COM64 and guard/DMA0 readback')
    ap.add_argument('--dma-peer',action='store_true',help='require COMP/COM/DMA64 with independent guard0')
    ap.add_argument('--low-hysteresis',action='store_true',help='require COMP2 low hysteresis readback; changes analog threshold')
    ap.add_argument('--static-comp',action='store_true',help='require specialized live comparator path provenance')
    ap.add_argument('--inline-guard',action='store_true',help='require guard constructor codegen provenance')
    ap.add_argument('--guard-install',action='store_true',help='require fresh-admission in-place guard build')
    ap.add_argument('--seed-div12',action='store_true',help='require exact bounded seed mean build')
    ap.add_argument('--reentry-clear-once',action='store_true',help='require recovery duplicate-clear experiment')
    ap.add_argument('--carrier-math',action='store_true',help='require prepare-only carrier compare')
    ap.add_argument('--bin-math',action='store_true',help='require exact division-free phase/timeline indexing')
    ap.add_argument('--scheduling-tail',action='store_true',help='require qualified IRQ chronology and CPU epoch agreement')
    ap.add_argument('--comp-overlap',action='store_true',help='require stop-context-only nested IRQ probe')
    ap.add_argument('--comp-critical',action='store_true',help='require bounded reference-service exclusion evidence')
    ap.add_argument('--comp-decisions',action='store_true',help='require completed-call decision tail')
    ap.add_argument('--qualification-window',action='store_true',help='require captured qualification-window diagnostics')
    ap.add_argument('--qualification-sparse',action='store_true',help='require RAM sparse tail bound to final observation')
    ap.add_argument('--qualification-direct',action='store_true',help='require RAM direct counters bound to final observation; does not qualify firmware timing')
    ap.add_argument('--qualification-reject',action='store_true',help='require QD85-v2 rejected-read positions')
    ap.add_argument('--quiet-irq-stamp',action='store_true',help='require report-only dispatch timestamp omitted')
    ap.add_argument('--masked-seed-arm',action='store_true',help='require masked fresh-seed setup provenance')
    ap.add_argument('--reentry-carrier',action='store_true',help='require early recovery carrier preparation provenance')
    ap.add_argument('--reentry-pwm-stage',action='store_true',help='require explicit recovery PWM staging candidate')
    ap.add_argument('--reentry-guard-stage',action='store_true',help='require cold guard staging with fresh final admission')
    ap.add_argument('--reentry-next-edge',action='store_true',help='require a real subsequent qualified edge after recovery setup')
    ap.add_argument('--follow-expected-phase',action='store_true',help='require expected-phase-only final follow wait')
    ap.add_argument('--prepared-handoff',action='store_true',help='require staged absolute-timer recovery build (original admission floors)')
    ap.add_argument('--follow-prevalidate',action='store_true',help='require speculative policy during confirmation')
    ap.add_argument('--follow-direct',action='store_true',help='require final sensing merged into handoff wait')
    ap.add_argument('--follow-setup-phase',action='store_true',help='require expected-phase-only postqualification setup')
    ap.add_argument('--follow-persistence',action='store_true',help='require twelve consecutive comparator reads for recovery follow')
    args=ap.parse_args()
    if sum((args.cycle360,args.cycle400,args.cycle450))>1:
        ap.error('select only one running cycle profile')
    if args.fast_cycle_report and not args.cycle450:ap.error('fast-cycle-report requires --cycle450')
    if args.event100 and not args.fast_cycle_report:ap.error('event100 requires --fast-cycle-report')
    if args.qualification_reject and not args.qualification_direct: ap.error('qualification-reject requires --qualification-direct')
    if args.qualification_window and args.core_trace!=0: ap.error('qualification-window requires --core-trace 0')
    if args.qualification_sparse and (args.core_trace!=0 or args.qualification_window):
        ap.error('qualification-sparse requires --core-trace 0 and no qualification-window')
    if args.qualification_direct and (args.core_trace!=0 or args.qualification_window or args.qualification_sparse or args.comp_paths or args.comp_decisions):
        ap.error('qualification-direct requires --core-trace 0 and no other qualification/path collector')
    if bool(args.out)==bool(args.infile): ap.error('choose --out OR --infile')
    if not 20<=args.ms<=600000: ap.error('finite window must be 20..600000 ms')
    if args.dropout and args.ms<=2000: ap.error('dropout needs --ms greater than2000')
    if args.reentry and not args.dropout: ap.error('reentry requires --dropout')
    if args.reentry_pwm_stage and not args.reentry: ap.error('reentry-pwm-stage requires --reentry')
    if args.reentry_guard_stage and not args.reentry_pwm_stage: ap.error('reentry-guard-stage requires --reentry-pwm-stage')
    if args.reentry_next_edge and not args.reentry_guard_stage: ap.error('reentry-next-edge requires --reentry-guard-stage')
    if args.follow_expected_phase and not args.reentry_next_edge: ap.error('follow-expected-phase requires --reentry-next-edge')
    if args.prepared_handoff and args.reentry and not args.reentry_next_edge: ap.error('prepared recovery requires --reentry-next-edge')
    if args.follow_prevalidate and not args.prepared_handoff: ap.error('prevalidation requires --prepared-handoff')
    if args.follow_direct and not args.follow_prevalidate: ap.error('direct follow requires --follow-prevalidate')
    if args.follow_setup_phase and not args.follow_direct: ap.error('setup phase requires --follow-direct')
    if args.follow_persistence and not args.follow_setup_phase: ap.error('follow persistence requires --follow-setup-phase')
    if args.seed450 and (args.seed400 or not args.follow_persistence): ap.error('seed450 requires persistence and excludes --seed400')
    if args.seed500 and (args.seed400 or args.seed450 or not args.follow_persistence): ap.error('seed500 requires persistence and excludes other seed profiles')
    if args.infile: text=args.infile.read_text()
    else:
        with args.out.open('xb'): pass
        target,steps=startup_plan(args.startup_direct)
        text=live_capture('COM41',115200,target,62,False,None,raw_path=args.out,
            step_targets=steps,catch_duty=62,capture_stride=19,
            driven=True,drive_duty=args.drive_duty,drive_phase=args.phase_shift,
            drive_handoff=True,engage_ms=args.ms,dropout=args.dropout,reentry=args.reentry,
            core_trace=args.core_trace,bemf_duty=args.bemf_duty)
    reject_pre_handoff_fault(text)
    if args.startup_direct: verify_direct_startup(text)
    from drv_reentry_pwm_stage import verify as verify_pwm_stage
    from drv_prepared_handoff import verify as verify_prepared_handoff
    verify_prepared_handoff(text,args.prepared_handoff)
    from drv_prepared_handoff import verify_prevalidation
    verify_prevalidation(text,args.follow_prevalidate)
    from drv_prepared_handoff import verify_direct
    verify_direct(text,args.follow_direct)
    from drv_prepared_handoff import verify_setup_phase
    verify_setup_phase(text,args.follow_setup_phase)
    from drv_prepared_handoff import verify_persistence
    verify_persistence(text,args.follow_persistence)
    verify_pwm_stage(text,args.reentry_pwm_stage,args.reentry_guard_stage,args.reentry_next_edge,args.follow_expected_phase)
    verify_seed_timing(text,args.seed_timing_reanchor)
    from drv_fast_cycle import verify as verify_fast_cycle
    verify_fast_cycle(text,args.fast_cycle_report)
    from drv_dma_guard import verify as verify_dma_guard
    if args.dma_guard or 'guard_irq=' in text:
        verify_dma_guard(text,args.dma_guard)
    verify_prestart_dma(text,args.prestart_dma)
    verify_cycle360(text,args.cycle360)
    verify_cycle400(text,args.cycle400)
    verify_cycle450(text,args.cycle450,args.event100)
    from drv_seed_profile import verify as verify_seed_profile
    verify_seed_profile(text,args.seed400,args.seed450,args.seed500)
    from drv_final_preparation import verify as verify_final_preparation
    verify_final_preparation(text,args.final_edge_prepare)
    verify_mode(text,required=args.pwm_roles,carrier_hz=args.carrier_hz)
    from drv_atomic_check import verify_backend
    verify_backend(text,single_core=True if args.single_core_atomics else None)
    if args.inline_comp and re.findall(r'^COMPREAD ([^\r\n]+)',text,re.M)!=['inline_adapter=1 sample_count_unchanged=1 signal_cached=0']:
        raise ValueError('missing comparator inlining provenance')
    result=verify(text,args.ms if args.out else None,dropout=args.dropout,reentry=args.reentry)
    if args.final_edge_prepare:
        verify_final_preparation(text,True,used=args.reentry)
    from drv_duty_split import verify as verify_split
    verify_split(text,acquisition=result['acquisition']['duty_tenths'],bemf=args.bemf_duty)
    verify_core_trace(text,args.core_trace)
    verify_read_call(text,args.comp_read_call)
    verify_com_keep_running(text,args.com_keep_running)
    if args.quiet_irq_stamp:
        verify_quiet_irq_stamp(text)
    if args.masked_seed_arm:
        from drv_seedmask_check import verify_mode as verify_seedmask
        verify_seedmask(text)
    if args.reentry_carrier:
        verify_reentry_carrier(text)
    verify_peer_priority(text,required=args.peer_priority,dma_peer=args.dma_peer,
                         prepared_recovery=args.prepared_handoff and args.reentry)
    verify_hysteresis(text,required=args.low_hysteresis)
    verify_static_comp(text,required=args.static_comp)
    verify_guard_codegen(text,required=args.inline_guard)
    verify_guard_install(text,required=args.guard_install)
    verify_seed_math(text,required=args.seed_div12)
    verify_reentry_clear(text,required=args.reentry_clear_once)
    verify_carrier_math(text,required=args.carrier_math)
    verify_bin_math(text,required=args.bin_math)
    from drv_scheduling_tail import verify_cpu
    chronology=verify_cpu(text,required=args.scheduling_tail)
    if chronology is not None:result['scheduling_tail']=chronology
    from drv_comp_overlap import decode as decode_overlap
    overlap=decode_overlap(text,required=args.comp_overlap)
    if overlap is not None:result['comp_overlap']=overlap
    from drv_comp_critical import decode as decode_critical
    critical=decode_critical(text,required=args.comp_critical)
    if critical is not None:result['comp_critical']=critical
    from drv_comp_decisions import decode as decode_decisions
    decisions=decode_decisions(text,required=args.comp_decisions)
    from drv_qualification_window import decode as decode_qualification
    qualification=decode_qualification(text,required=args.qualification_window)
    if qualification is not None:result['qualification_window']=qualification
    from drv_qualification_sparse import decode_campaign
    sparse=decode_campaign(text,required=args.qualification_sparse)
    if sparse is not None:result['qualification_sparse']=sparse
    from drv_qualification_direct import decode_campaign as decode_direct_campaign
    direct=decode_direct_campaign(text,required=args.qualification_direct)
    if args.qualification_reject and (direct is None or direct['wire_version']!=2):
        raise ValueError('missing direct rejection-position format v2')
    if direct is not None:result['qualification_direct']=direct
    if decisions is not None:result['comp_decisions']=decisions
    if args.comp_paths:
        from drv_comp_paths import decode as decode_paths
        from drv_architecture_report import report as architecture_report
        decode_paths(text,required=True)
        result['architecture']=architecture_report(text,reentry=args.reentry)
    if args.out and (result['acquisition']['duty_tenths']!=args.drive_duty or
                     result['acquisition']['phase_shift_degrees']!=args.phase_shift):
        raise ValueError('applied acquisition settings differ from requested experiment')
    print(json.dumps(result,indent=2))


if __name__=='__main__': main()
