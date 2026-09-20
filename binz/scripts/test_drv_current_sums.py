import base64
import struct
import unittest
import zlib
from drv_current_sums import decode


def capture(n=2,last=202,totals=(-2,2,0,2400,3000)):
    text='ADCSTATS uncalibrated=1 segment_only=1 current_center=2048\nPOWERFEEDBACK scans=2 peak_abs_raw=1\n'
    for ch,total in enumerate(totals):
        row=struct.pack('<7Hq',ch,n,0,101,0,last,0,total)
        text+='S85 '+base64.a85encode(row+struct.pack('<I',zlib.crc32(row))).decode()+'\n'
    return text


class SumsTests(unittest.TestCase):
    def test_real_irq_guard_capture_and_current_stop(self):
        from pathlib import Path
        from drv_sustained_report import summarize
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,n,reason in [('dmaguard_625_hold70_10s.txt',49751,'2'),
                              ('dmaguard_625_ramp250_30s.txt',36776,'5')]:
            text=(root/name).read_text()
            sums=decode(text)
            self.assertTrue(sums['irq_guard'])
            self.assertEqual(sums['n'],n)
            self.assertEqual(sums['unaggregated_publications'],0)
            row=summarize(text)
            self.assertEqual(row['powered_reason'],reason)
            self.assertEqual(row['outputs_off_verified'],1)

    def test_irq_publication_tail_is_explicit_and_bounded(self):
        stream='DMAFEEDBACK trigger_us=101 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=40 queue_peak=3 guard_irq=1\n'
        for missing in range(5):
            text=capture().replace('scans=2',f'scans={2+missing}')+stream
            result=decode(text)
            self.assertEqual(result['unaggregated_publications'],missing)
            self.assertFalse(result['coverage_proven'])
        for n in [1,7]:
            with self.assertRaises(ValueError):decode(capture().replace('scans=2',f'scans={n}')+stream)
        with self.assertRaises(ValueError):
            decode(capture().replace('scans=2','scans=3')+stream.replace('guard_irq=1','guard_irq=0'))

    def test_lean_irq_omitted_publication_counter_does_not_erase_sums(self):
        stream='DMAFEEDBACK trigger_us=209 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=0 queue_peak=0 guard_irq=1\n'
        lean='LEANCORE r1 recorder=0 comp_max=0 com_max=0 control_progress=1 lean_irq=1\n'
        text=capture(last=310).replace('scans=2','scans=0')+stream+lean
        self.assertEqual(decode(text)['n'],2)
        with self.assertRaises(ValueError): decode(text.replace(lean,''))

    def test_real_fast_start_recovery(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/fast_start_reentry48_01.txt').read_text()
        self.assertTrue(verify(text,10000,dropout=True,reentry=True)['powered_recovery_verified'])
        self.assertIn('DMASTART first_trigger_us=1 steady_trigger_us=201',text)
        with self.assertRaises(ValueError):decode(text.replace('decision_us=246','decision_us=351'))

    def test_real_early_trigger_recovery(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/early_trigger_reentry45_01.txt').read_text()
        self.assertEqual(decode(text)['n'],39739)
        self.assertTrue(verify(text,10000,dropout=True,reentry=True)['powered_recovery_verified'])
        for bad in [text.replace('decision_us=301','decision_us=400'),text.replace('acquired_us=142','acquired_us=143')]:
            with self.assertRaises(ValueError):decode(bad)

    def test_startup_cadence_metadata(self):
        stream='DMAFEEDBACK trigger_us=201 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=10 queue_peak=1\n'
        start='DMASTART first_trigger_us=101 steady_trigger_us=201 preloaded_counter=1\n'
        self.assertEqual(decode(capture(last=302)+stream+start)['trigger_us'],201)
        self.assertEqual(decode(capture(last=302)+stream+start.replace('first_trigger_us=101','first_trigger_us=1'))['trigger_us'],201)
        for bad in [capture(last=302)+start,capture(last=302)+stream+start+start,
                    capture(last=302)+stream+start.replace('steady_trigger_us=201','steady_trigger_us=101')]:
            with self.assertRaises(ValueError):decode(bad)
    def test_real_recovery_initial_gap_failure(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/adc_phase_reentry48_01.txt').read_text()
        self.assertEqual(decode(text)['n'],1)
        with self.assertRaises(ValueError):verify(text,10000,dropout=True,reentry=True)
        with self.assertRaises(ValueError):decode(text.replace('acquired_us=147 fault=4','acquired_us=147 fault=0'))

    def test_restart_bug_and_fixed_hold(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        root=Path(__file__).resolve().parents[1]/'captures'
        with self.assertRaises(ValueError):decode((root/'driven_dma201_hold45_01.txt').read_text())
        text=(root/'driven_dma201_reset_hold45_01.txt').read_text()
        self.assertEqual(decode(text)['n'],49749)
        self.assertTrue(verify(text,10000)['powered_handoff_window_verified'])
        recovery=(root/'driven_dma201_reset_reentry45_01.txt').read_text()
        self.assertEqual(decode(recovery)['n'],1)
        with self.assertRaises(ValueError):verify(recovery,10000,dropout=True,reentry=True)

    def test_real_201us_handoff(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/driven_dma201_45_01.txt').read_text()
        result=decode(text)
        self.assertEqual((result['trigger_us'],result['n'],result['first_us'],result['last_us']),(201,4974,253,999826))
        self.assertTrue(verify(text,1000)['powered_handoff_window_verified'])
        with self.assertRaises(ValueError):decode(text.replace('trigger_us=201','trigger_us=101'))

    def test_explicit_slower_cadence(self):
        stream='DMAFEEDBACK trigger_us=201 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=11 queue_peak=3\n'
        self.assertEqual(decode(capture(last=302)+stream)['trigger_us'],201)
        for bad in [capture()+stream,capture(last=302),capture(last=302)+stream+stream,
                    capture(last=302)+stream.replace('201','200')]:
            with self.assertRaises(ValueError):decode(bad)

    def test_explicit_phase_walk_cadence(self):
        stream='DMAFEEDBACK trigger_us=209 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=11 queue_peak=3\n'
        start='DMASTART first_trigger_us=1 steady_trigger_us=209 preloaded_counter=1\n'
        self.assertEqual(decode(capture(last=310)+stream+start)['trigger_us'],209)
        for bad in [capture()+stream, capture(last=310)+stream.replace('209','208'),
                    capture(last=310)+stream+start.replace('steady_trigger_us=209','steady_trigger_us=201')]:
            with self.assertRaises(ValueError): decode(bad)

        distributed=stream.replace('209','226')
        distributed_start=start.replace('209','226')
        self.assertEqual(decode(capture(last=327)+distributed+distributed_start)['trigger_us'],226)

    def test_real_combined_stream_failure_is_not_motor_pass(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/driven_dma45_01.txt').read_text()
        result=decode(text)
        self.assertEqual((result['n'],result['first_us'],result['last_us']),(97,153,9849))
        self.assertTrue(result['sequential_tim3_declared'])
        with self.assertRaises(ValueError):verify(text,1000)

    def test_sequential_owner_protocol(self):
        owner='DMAOWNER sequential_tim3=1 forced_release_required=1 refusal_code=11\n'
        stream='DMAFEEDBACK trigger_us=101 acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=10 queue_peak=4\n'
        self.assertTrue(decode(capture()+owner+stream)['sequential_tim3_declared'])
        self.assertFalse(decode(capture())['sequential_tim3_declared'])
        for bad in [capture()+owner,capture()+owner+owner+stream,
                    (capture()+owner+stream).replace('forced_release_required=1','forced_release_required=0')]:
            with self.assertRaises(ValueError):decode(bad)

    def test_signed_and_no_current_claim(self):
        result=decode(capture());self.assertEqual(result['signed_raw_sums'],[-2,2,0])
        self.assertFalse(result['calibrated_current'])
    def test_missing_duplicate_corrupt(self):
        text=capture();lines=text.splitlines()
        for bad in ['\n'.join(lines[:-1]),text+lines[-1]+'\n',text.replace('S85 ','S85 !',1)]:
            with self.assertRaises(ValueError): decode(bad)
    def test_time_count_and_range(self):
        for text in [capture(last=203),capture(n=3,last=303),capture(totals=(-5000,2,0,2400,3000))]:
            with self.assertRaises(ValueError): decode(text)
