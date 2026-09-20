"""Synthetic protocol/clock tests, not motor or ownership qualification."""
import re
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch
import drv_capture
from drv_driven_handoff import transfer_provenance,verify,first_arm,verify_core_trace
from drv_driven_run import records,verify as verify_observer
from test_drv_driven_run import row,fixture
from test_drv_capture import FakePort,OffReadbackTests

class PeerPriorityTests(unittest.TestCase):
    def test_guard_install_provenance(self):
        from drv_driven_handoff import verify_guard_install
        good='GUARDINSTALL admission_token=1 fresh_checks=1 in_place=1 prestaged=0\n'
        verify_guard_install(good,True);verify_guard_install('legacy\n')
        for bad in ['',good+good,'GUARDINSTALL\n',good.replace('fresh_checks=1','fresh_checks=0')]:
            with self.assertRaises(ValueError):verify_guard_install(bad,True)
    def test_inline_guard_provenance(self):
        from drv_driven_handoff import verify_guard_codegen
        good='GUARDCODE inline_constructor=1 admission_unchanged=1 prestaged=0\n'
        verify_guard_codegen(good,True);verify_guard_codegen('legacy\n')
        for bad in ['',good+good,'GUARDCODE\n',good.replace('prestaged=0','prestaged=1')]:
            with self.assertRaises(ValueError):verify_guard_codegen(bad,True)
    def test_static_comp_provenance(self):
        from drv_driven_handoff import verify_static_comp
        good='COMPSTATIC real_inverted_traceoff=1 live_reads=1 reference_core=1 diagnostic_fallback=1\n'
        verify_static_comp(good,True);verify_static_comp('legacy\n')
        for bad in ['', 'COMPSTATIC\n',good+good,good.replace('live_reads=1','live_reads=0')]:
            with self.assertRaises(ValueError):verify_static_comp(bad,True)
    def test_low_hysteresis_readback_and_legacy(self):
        from drv_driven_handoff import verify_hysteresis
        good='COMPHYST code=1 startup_and_bemf=1 raw_reads_unchanged=1\n'
        verify_hysteresis(good,True);verify_hysteresis('legacy\n')
        for bad in ['',good+good,good.replace('code=1','code=0'),good.replace('code=1','code=2')]:
            with self.assertRaises(ValueError):verify_hysteresis(bad,True)
    def test_expected_or_legacy(self):
        from drv_driven_handoff import verify_peer_priority
        verify_peer_priority('COREPRIORITY comp=64 com=64 guard=0 dma=0\n',True)
        verify_peer_priority('legacy capture\n')
        with self.assertRaises(ValueError):verify_peer_priority('legacy capture\n',True)
    def test_guard_demotion_or_source_mismatch(self):
        from drv_driven_handoff import verify_peer_priority
        for body in ['comp=64 com=128 guard=0 dma=0','comp=64 com=64 guard=64 dma=0','comp=64 com=64 guard=0 dma=64']:
            with self.subTest(body=body),self.assertRaises(ValueError):verify_peer_priority('COREPRIORITY '+body+'\n')


def provenance():
    return '\n'.join([
        'DRIVEX result=1 power_reason=2 fresh_transfer=1',
        'DX85FIELDS step,edge_half_us,mean_half_us,release_us,foreground_us,feedback_age_us,foreground_entered',
        row('DX85',[2,20000,1600,10030,10050,150,1]),
        row('DS85',[1,2,20000,0,1600,12,7,9600,9600,0]),
        row('DA85',[9900,100,2048,2048,2048,11700,1500]),
        'CORESEED assumed=0 source=measured_flying interval_ticks=1600 edge_age_ticks=120 remaining_arr=280 arm_us=8 bootstrap_com=1 synthetic_accept=0',
        'CORESEED armed=1 refusal_stop_code=8'])


def synthetic_complete():
    # Intentionally splice TWO DIFFERENT archived experiments in memory to
    # exercise the entire decoder. This is NEVER a motor/handoff artifact.
    root=Path(__file__).resolve().parents[1]/'captures'
    source=(root/'driven_seed_full60_01.txt').read_text()
    irq=records(source,'DI85',7)[:13];last=irq[-1];stop=last[3]+40
    release=last[3]+20;foreground=stop+10
    commands=[r for r in records(source,'DC85',3) if r[0]<=last[0]]
    q=[r for r in records(source,'DQ85',4) if r[0]<stop]
    adc=[r for r in records(source,'DA85',7) if r[0]+r[1]<stop]
    bounds=records(source,'PD85',3)[:len(commands)]
    pwm=records(source,'PC85',3)[:bounds[-1][2]+3]
    s=verify_observer(source);s.update(reason=22,stop_us=stop,commands=len(commands)-1,
        reads=len(q),scans=len(adc),candidates=sum((r[2]>>4)&3==1 for r in q))
    labels=('DRIVEPHASE ','DI85FIELDS ','DRIVENSEED ')
    lines=[line for line in source.splitlines() if line.startswith(labels)]
    lines+=['DRIVEOBS '+' '.join(f'{k}={s[k]}' for k in (
        'reason','start_us','stop_us','theta','rate','duty_tenths','first_us','commands','reads','scans',
        'tick_max_us','sector_max_us','late_max_us','age_max_us','candidates','missed','handoff_authority','disabled')),
        'DRIVENCOAST unavailable=1 acquisition_release_not_final_stop=1',
        f"DRIVENIRQ calls=200 accepts=13 max_us={s['irq_max_us']} rate_peak=45 rate_limit=64 fixed_average_ticks=1666 filter_reads=12 raw_inverted=1 handoff_authority=0 masked=1",
        f'PWMCOMP n={len(pwm)} target=192 flags={0 if len(pwm)<128 else 5} stopped=1 source_comp2_csr=1 handoff_authority=0']
    for label,rows in [('DQ85',q),('DA85',adc),('DC85',commands),('PD85',bounds),('PC85',pwm),('DI85',irq),('DS85',records(source,'DS85',10))]:
        lines.extend(row(label,r) for r in rows)
    seed=records(source,'DS85',10)[0];ci=seed[4];age=2*(foreground+10)-seed[2]
    lines+=['DRIVEOBS END','DRIVEX result=1 power_reason=2 fresh_transfer=1',
        'DX85FIELDS step,edge_half_us,mean_half_us,release_us,foreground_us,feedback_age_us,foreground_entered',
        row('DX85',[seed[1],seed[2],ci,release,foreground,foreground-adc[-1][0],1]),
        f'CORESEED assumed=0 source=measured_flying interval_ticks={ci} edge_age_ticks={age} remaining_arr={ci//2-((ci*16)>>6)-age} arm_us=8 bootstrap_com=1 synthetic_accept=0',
        'CORESEED armed=1 refusal_stop_code=8']
    powered=(root/'range300_46_01.txt').read_text()
    powered='\n'.join(line for line in powered.splitlines() if not line.startswith('CORESEED '))
    return '\n'.join(['SYNTHETIC COMPOSITE NOT HARDWARE EVIDENCE',*lines,powered,'FINALOFF',
        OffReadbackTests.GOOD.decode(),'STACK span=7260 painted=5988 untouched=5500 diagnostic_only=1'])


class CoreTraceEvidence(unittest.TestCase):
    def test_cached_traceoff_three_hold_manifest(self):
        import csv
        import hashlib
        root=Path(__file__).resolve().parents[1]/'captures'
        rows=list(csv.DictReader((root/'cachedoff_hold55_cohort.csv').read_text().splitlines()))
        self.assertEqual(len(rows),3)
        for r in rows:
            data=(root/r['capture']).read_bytes();text=data.decode()
            self.assertEqual(hashlib.sha256(data).hexdigest(),r['sha256'])
            verify_core_trace(text,0)
            result=verify(text,10000)
            self.assertEqual(result['acquisition']['duty_tenths'],55)
            self.assertIn('COMPMODE cached_per_call=1 signal_cached=0 safety_cached=0',text)
            for field in ('commutations','accepted_events','peak_abs_raw','bus_min_mv'):
                self.assertEqual(str(result['powered'][field]),r[field])
            self.assertAlmostEqual(result['powered']['accepted_rate_ehz'],float(r['mean_ehz']))
            self.assertAlmostEqual(result['powered']['cycle_sigma_us'],float(r['cycle_sigma_us']))

    def test_explicit_modes_and_inherited_mode(self):
        verify_core_trace('',None)
        for mode in (0,1):
            verify_core_trace(f'CORETRACE enabled={mode} per_read_bookkeeping={mode}',mode)

    def test_missing_mismatched_and_duplicate_rejected(self):
        marker='CORETRACE enabled=0 per_read_bookkeeping=0'
        for text in ('',marker+'\n'+marker,
                     'CORETRACE enabled=1 per_read_bookkeeping=1',
                     'CORETRACE enabled=0 per_read_bookkeeping=1'):
            with self.subTest(text=text),self.assertRaises(ValueError):
                verify_core_trace(text,0)

    def test_actual_capture_modes(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,mode in [('traceoff_hold54_01.txt',0),('traceoff_hold55_01.txt',0),
                          ('cycle_core_hold54_01.txt',1)]:
            verify_core_trace((root/name).read_text(),mode)


class TransferEvidence(unittest.TestCase):
    def test_cached_traceoff_recovery_controls_keep_original_budget(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,ms,spare in [('cachedoff_reentry54_01.txt',10000,227),
                             ('cachedoff_reentry54_30s_01.txt',30000,216)]:
            text=(root/name).read_text()
            verify_core_trace(text,0)
            result=verify(text,ms,dropout=True,reentry=True)
            self.assertTrue(result['powered_recovery_verified'])
            p=result['powered']
            self.assertEqual(int(p['original_end_elapsed_us'])-int(p['final_elapsed_us']),spare)
            self.assertEqual(p['recovery_seed_ticks'],'1167')

    def test_actual_initial_cycle_abort_is_not_recovery_staging_failure(self):
        text=(Path(__file__).resolve().parents[1]/'captures/cachedoff_reentry55_01.txt').read_text()
        with self.assertRaisesRegex(ValueError,'stopped before dropout; recovery not exercised; reason=12'):
            verify(text,10000,dropout=True,reentry=True)
        from drv_sustained_report import summarize
        result=summarize(text)
        self.assertEqual(result['outputs_off_verified'],1)
        self.assertEqual(result['reentry_verified'],0)

    def test_actual_three_recoveries_keep_strict_original_deadline(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for n,margin in [(1,145),(2,198),(3,232)]:
            text=(root/f'driven_reserve45_{n:02}.txt').read_text()
            result=verify(text,10000,dropout=True,reentry=True)
            self.assertTrue(result['powered_recovery_verified'])
            p=result['powered']
            self.assertEqual(int(p['original_end_elapsed_us'])-int(p['final_elapsed_us']),margin)
            self.assertIn('REENTRYRESERVE us=300 included_in_original_deadline=1',text)
            for bad in [text.replace('REENTRYRESERVE us=300','REENTRYRESERVE us=200'),
                        text.replace('REENTRYRESERVE us=300','REENTRYRESERVE us=301'),
                        text.replace('REENTRYRESERVE us=300 included_in_original_deadline=1',''),
                        text+'\nREENTRYRESERVE us=300 included_in_original_deadline=1\n']:
                with self.assertRaises(ValueError): verify(bad,10000,dropout=True,reentry=True)
    def test_actual_recovery_preserves_initial_arm_and_rejects_late_finish(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['driven_reentry45_01','driven_reentry45_02','driven_reentry45_03','driven_reentry45_04']:
            with self.assertRaises(ValueError): verify((root/(name+'.txt')).read_text(),10000,dropout=True,reentry=True)
        text=(root/'driven_reentry45_04.txt').read_text()
        first=first_arm(text)
        self.assertEqual(first[5]+first[6],14712086)
        self.assertIn('final_elapsed_us=14712092',text)
        self.assertIn('remaining_arr=70 arm_us=13',text)
        self.assertIn('POWERCOMMITS applied=11285',text)
        # Initial provenance remains independently valid even though the
        # resumed window misses the original campaign deadline by6us.
        acq=verify_observer(text,transfer=True)
        self.assertGreaterEqual(transfer_provenance(text,acq,reentry=True)['remaining_half_us'],64)
        # Relabeling the old reserve cannot repair an actually late finish.
        for reserve in [200,300]:
            with self.assertRaises(ValueError):
                verify(text+f'\nREENTRYRESERVE us={reserve} included_in_original_deadline=1\n',10000,dropout=True,reentry=True)
    def test_initial_arm_archive_is_distinct_from_recovery_seed(self):
        values=[1600,120,280,8,1,4712000,10000000]
        words=[w for v in values for w in [v&65535,v>>16]]
        header='DRIVENFIRST fields=interval_ticks,edge_age_ticks,remaining_arr,arm_us,armed,first_elapsed_us,requested_us u32_le_pairs=1 before_reentry=1'
        text=provenance().replace('interval_ticks=1600','interval_ticks=1435')+'\n'+header+'\n'+row('DFA85',words)
        self.assertEqual(first_arm(text),values)
        self.assertEqual(transfer_provenance(text,dict(start_us=0,stop_us=10040),reentry=True)['seed_ticks'],1600)
        with self.assertRaises(ValueError): transfer_provenance(text,dict(start_us=0,stop_us=10040))
        with self.assertRaises(ValueError): first_arm(text+'\n'+row('DFA85',words))
        bad=words.copy();bad[8]=0
        with self.assertRaises(ValueError): first_arm(text.replace(row('DFA85',words),row('DFA85',bad)))
    def test_actual_tracking_loss_archive_and_same_boot_control(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'driven_drop45_01.txt').read_text()
        result=verify(text,10000,dropout=True)
        self.assertTrue(result['tracking_loss_stop_verified'])
        self.assertFalse(result['powered_handoff_window_verified'])
        self.assertEqual(result['powered']['dropout_to_disabled_observation_us'],777)
        self.assertEqual(result['powered']['reentry_verified'],0)
        with self.assertRaises(ValueError): verify(text,10000)
        for old,new in [('FIRSTSEG fault=8','FIRSTSEG fault=5'),
                        ('DROPOUT applied=1','MISSINGDROP applied=1'),
                        ('automatic_restart=0','automatic_restart=1')]:
            with self.assertRaises(ValueError): verify(text.replace(old,new),10000,dropout=True)
        control=(root/'driven_drop45_control_02.txt').read_text()
        self.assertTrue(verify(control,3000)['powered_handoff_window_verified'])
        self.assertNotIn('DROPOUT applied=1',control)
        with self.assertRaises(ValueError): verify(control,3000,dropout=True)
        with self.assertRaises(ValueError): verify((root/'driven_drop45_control_01.txt').read_text())
    def test_actual_four_attempt_alignment_cohort(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for n in range(1,5):
            text=(root/f'driven_align45_{n:02}.txt').read_text()
            result=verify(text,10000)
            self.assertTrue(result['powered_handoff_window_verified'])
            self.assertEqual(result['acquisition']['duty_tenths'],45)
            self.assertGreaterEqual(result['transfer']['remaining_half_us'],94)
            if n==1:
                self.assertEqual(result['acquisition']['initial_remaining_us'],18)
                self.assertEqual(result['acquisition']['alignment_wait_us'],7)
            for old,new in [('gates_disabled_during_wait=1','gates_disabled_during_wait=0'),
                            ('elapsed_phase=1','elapsed_phase=0')]:
                with self.assertRaises(ValueError): verify(text.replace(old,new))
        for old,new in [('requested_wait_us=0','requested_wait_us=1'),
                        ('actual_wait_us=0','actual_wait_us=200')]:
            with self.assertRaises(ValueError): verify(text.replace(old,new))
    def test_actual_scan_yield_and_long_window_keep_original_feedback(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,ms,com,events in [('driven_yield45_01',1000,'1409','1408'),
                                   ('driven_yield45_03',10000,'14157','14156')]:
            text=(root/(name+'.txt')).read_text();result=verify(text,ms)
            self.assertEqual(result['powered']['commutations'],com)
            self.assertEqual(result['powered']['accepted_events'],events)
            self.assertFalse(result['independent_bemf_lock_proven'])
            with self.assertRaises(ValueError): verify(text.replace('partial_feedback_published=0','partial_feedback_published=1'))
        partial=(root/'driven_yield45_01.txt').read_text()
        self.assertIn('abandoned=1 completed_channels=1',partial)
        for name in ['driven_yield45_02','driven_yield46_01']:
            with self.assertRaises(ValueError): verify((root/(name+'.txt')).read_text())
    def test_actual_first_completed_window_and_same_build_refusals(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'driven_handoff46_05.txt').read_text();result=verify(text,20)
        self.assertEqual(result['transfer']['remaining_half_us'],124)
        self.assertEqual(result['powered']['commutations'],'27')
        self.assertEqual(result['powered']['accepted_events'],'26')
        self.assertFalse(result['independent_bemf_lock_proven'])
        with self.assertRaises(ValueError): verify(text,1000)
        for name in ['driven_handoff46_04','driven_handoff46_06']:
            failed=(root/(name+'.txt')).read_text()
            self.assertTrue(verify_observer(failed,transfer=True)['driven_seed_ready'])
            with self.assertRaises(ValueError): verify(failed)
    def test_actual_armed_but_frozen_interval_is_not_completion(self):
        text=(Path(__file__).resolve().parents[1]/'captures/driven_handoff46_03.txt').read_text()
        acquisition=verify_observer(text,transfer=True)
        transfer=transfer_provenance(text,acquisition)
        self.assertEqual(transfer['remaining_half_us'],113)
        self.assertIn('interval_cnt=286',text)
        self.assertIn('edge_age_ticks=286',text)
        self.assertIn('POWERCOMMITS applied=1',text)
        with self.assertRaises(ValueError): verify(text)
    def test_actual_first_transfer_refused_expired_core_arm(self):
        text=(Path(__file__).resolve().parents[1]/'captures/driven_handoff46_01.txt').read_text()
        acq=verify_observer(text,transfer=True)
        self.assertEqual(acq['pwm_dma_samples'],115)
        self.assertTrue(acq['driven_seed_ready'])
        self.assertIn('CORESEED armed=0',text)
        self.assertIn('POWERCOMMITS applied=0',text)
        with self.assertRaises(ValueError): verify(text)
        with self.assertRaises(ValueError): verify_observer(text)
        for flags in [1,2,4,5,8,15]:
            with self.assertRaises(ValueError):
                verify_observer(text.replace('n=115 target=192 flags=0',f'n=115 target=192 flags={flags}'),transfer=True)
    def test_complete_decoder_with_explicitly_synthetic_composite(self):
        text=synthetic_complete();result=verify(text)
        self.assertTrue(result['powered_handoff_window_verified'])
        self.assertFalse(result['independent_bemf_lock_proven'])
        with self.assertRaises(ValueError): verify_observer(text)
        with self.assertRaises(ValueError): verify(text,20)
        for old,new in [('POWERPATH reason=2','POWERPATH reason=8'),
                        ('POWERPATH ','MISSINGPOWER '),('CORESEED armed=1','CORESEED armed=0'),
                        ('ACCEPTQUALITY ','MISSINGQUALITY '),('TIMELINE label=ET85','TIMELINE label=BAD')]:
            with self.subTest(old=old),self.assertRaises((ValueError,RuntimeError)):
                verify(text.replace(old,new))
        line=next(line for line in text.splitlines() if line.startswith('POWERPATH '))
        with self.assertRaises(ValueError): verify(text+'\n'+line)

    def test_seed_identity_and_fresh_arm(self):
        result=transfer_provenance(provenance(),dict(start_us=0,stop_us=10040))
        self.assertEqual(result['remaining_half_us'],280)
        self.assertEqual(result['release_to_foreground_us'],20)

    def test_stale_or_fabricated_transfer_is_rejected(self):
        original=provenance()
        dx=row('DX85',[2,20000,1600,10030,10050,150,1])
        mutations=[original.replace(a,b) for a,b in [
            ('result=1','result=2'),('armed=1','armed=0'),('arm_us=8','arm_us=17'),
            ('remaining_arr=280','remaining_arr=63'),('edge_age_ticks=120','edge_age_ticks=98')]]
        for i,value in [(0,3),(1,20002),(2,1599),(3,10060),(4,11100),(5,1001),(5,149),(6,0)]:
            values=[2,20000,1600,10030,10050,150,1];values[i]=value
            mutations.append(original.replace(dx,row('DX85',values)))
        mutations.extend([original+'\n'+dx,original+'\nCORESEED armed=1 refusal_stop_code=8'])
        for text in mutations:
            with self.subTest(text=text),self.assertRaises(ValueError):
                transfer_provenance(text,dict(start_us=0,stop_us=10040))

    def test_observer_completion_is_not_powered_handoff(self):
        with self.assertRaises(ValueError): verify(fixture())
        with self.assertRaises(ValueError): verify(fixture().replace('reason=2 ','reason=22 '))


class TransferHandshake(unittest.TestCase):
    def exercise(self,missing=None,dropout=False,reentry=False,tracking_trip=False):
        port=FakePort()
        def reply(*args):
            cmd=port.commands[-1]
            if cmd==missing: return b''
            replies={
                'p':OffReadbackTests.GOOD,'i':OffReadbackTests.GOOD,
                'drivephase60':b'DRIVEPHASE target=60 accepted=1 one_shot=1 gate_authority=0',
                'drivedu46':b'DRIVEDUTY target=46 accepted=1 one_shot=1 gate_authority=0',
                'drivepwm192':b'DRIVEPWM target=192 accepted=1 gate_authority=0',
                'driveobs1':b'DRIVEOBS armed=1 window_us=20000 handoff_authority=0 one_shot=1',
                'engagems20':b'ENGAGEWINDOW ms=20',
                'engagems3000':b'ENGAGEWINDOW ms=3000',
                'drivedropms2000':b'DRIVEDROPWINDOW ms=2000 range=2000..10000 idle_only=1',
                'drivedrop1':b'DRIVEDROP armed=1 configured_window=1 stop_only=1 one_shot=1',
                'drivetrack1':b'DRIVETRACK armed=1 configured_window=1 immediate_trip=1 one_shot=1',
                'drivereentry1':b'DRIVEREENTRY armed=1 tracking_only=1 attempts_max=1 one_shot=1',
                'drivex1':b'DRIVEX armed=1 accepted=1 one_shot=1',
                'cap1':b'CAPTURE armed one-shot a85-v1'}
            return replies.get(cmd,b'')+b'\n'
        with patch.dict(sys.modules,serial=types.SimpleNamespace(Serial=lambda *a,**k:port)),\
             patch.object(drv_capture,'read_available',side_effect=reply),\
             patch.object(drv_capture.time,'sleep'):
            kwargs=dict(driven=True,drive_handoff=True,drive_duty=46,drive_phase=60,
                engage_ms=3000 if dropout or tracking_trip else 20,dropout=dropout,
                reentry=reentry,tracking_trip=tracking_trip)
            if missing:
                with self.assertRaises(RuntimeError): drv_capture.live_capture('FAKE',115200,50,62,False,None,**kwargs)
            else: drv_capture.live_capture('FAKE',115200,50,62,False,None,**kwargs)
        self.assertTrue(port.closed)
        self.assertIn('drivex0',port.commands);self.assertIn('engagems1000',port.commands)
        return port.commands

    def test_driven_dropout_requires_its_own_ack_and_disarms(self):
        commands=self.exercise(dropout=True)
        self.assertLess(commands.index('drivedrop1'),commands.index('run50'))
        self.assertLess(commands.index('drivedropms2000'),commands.index('drivedrop1'))
        self.assertIn('drivedrop0',commands)
        self.assertNotIn('dropout1',commands)
        self.assertNotIn('run50',self.exercise(missing='drivedrop1',dropout=True))
        self.assertNotIn('run50',self.exercise(missing='drivedropms2000',dropout=True))

    def test_driven_reentry_requires_separate_ack_and_disarms(self):
        commands=self.exercise(dropout=True,reentry=True)
        self.assertLess(commands.index('drivereentry1'),commands.index('run50'))
        self.assertIn('drivereentry0',commands)
        self.assertNotIn('reentry1',commands)
        self.assertNotIn('run50',self.exercise(missing='drivereentry1',dropout=True,reentry=True))

    def test_immediate_tracking_trip_is_explicit_exclusive_and_disarmed(self):
        commands=self.exercise(tracking_trip=True)
        self.assertLess(commands.index('drivetrack1'),commands.index('run50'))
        self.assertIn('drivetrack0',commands);self.assertNotIn('drivedrop1',commands)
        self.assertNotIn('run50',self.exercise(missing='drivetrack1',tracking_trip=True))

    def test_explicit_arm_precedes_motor_command(self):
        commands=self.exercise()
        self.assertLess(commands.index('engagems20'),commands.index('drivex1'))
        self.assertLess(commands.index('drivex1'),commands.index('run50'))

    def test_missing_handshake_never_starts_motor(self):
        for missing in ['engagems20','drivex1']:
            self.assertNotIn('run50',self.exercise(missing))

    def test_invalid_mode_or_window_never_opens_port(self):
        with patch.dict(sys.modules,serial=types.SimpleNamespace(Serial=lambda *a,**k:self.fail('opened port'))):
            for driven,ms in [(False,20),(True,19),(True,600001)]:
                with self.assertRaises(ValueError):
                    drv_capture.live_capture('FAKE',115200,50,62,False,None,driven=driven,drive_handoff=True,engage_ms=ms)


if __name__=='__main__': unittest.main()
