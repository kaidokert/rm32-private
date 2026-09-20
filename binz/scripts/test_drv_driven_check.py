import unittest
from pathlib import Path
from drv_driven_check import verify

class DrivenCheckTests(unittest.TestCase):
    def test_phase_boundary_schedule_matches_independent_waveform_calculation(self):
        text=(Path(__file__).resolve().parents[1]/'captures/drivenphase_01.txt').read_text()
        self.assertEqual(len(verify(text,real=True,comp=True,phased=True)),3)
        for changed in [text.replace('first_us=75','first_us=833',1),
                        text.replace('next_deadline_us=20075','next_deadline_us=20000',1),
                        text.replace('late_max_us=12','late_max_us=51',1),
                        text.replace('synthetic_phase=1','synthetic_phase=0',1)]:
            with self.assertRaises((ValueError,RuntimeError)): verify(changed,real=True,comp=True,phased=True)

    def test_shared_stop_and_new_disabled_session(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'drivenstop_01.txt').read_text()
        self.assertEqual(len(verify(text,real=True,comp=True,cancel=True)),3)
        self.assertEqual(len(verify((root/'drivencomp_afterstop_01.txt').read_text(),real=True,comp=True)),3)
        for changed in [text.replace('commands_unchanged=1','commands_unchanged=0',1),
                        text.replace('timers_off=1','timers_off=0',1),
                        text.replace('owner_off=1','owner_off=0',1),
                        text.replace('reason=9','reason=2',1),
                        text.replace('late_callbacks=2','late_callbacks=0',1)]:
            with self.assertRaises((ValueError,RuntimeError)): verify(changed,real=True,comp=True,cancel=True)

    def test_comparator_schedule_is_not_rotor_qualification(self):
        text=(Path(__file__).resolve().parents[1]/'captures/drivencomp_01.txt').read_text()
        self.assertEqual(len(verify(text,real=True,comp=True)),3)
        # Nonzero candidates with gates OFF are retained, not labeled BEMF.
        self.assertIn('candidates=5',text)
        for changed in [text.replace('gap_max_us=63','gap_max_us=101',1),
                        text.replace('bracket_max_us=1','bracket_max_us=3',1),
                        text.replace('reject_epoch=0','reject_epoch=1',1),
                        text.replace('steps_mask=63','steps_mask=31',1),
                        text.replace('synthetic_sectors=1','synthetic_sectors=0',1)]:
            with self.assertRaises((ValueError,RuntimeError)): verify(changed,real=True,comp=True)

    def test_real_adc_trials_and_startup_regression(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'drivenadc_04.txt').read_text()
        self.assertEqual(len(verify(text,real=True)),3)
        self.assertEqual(len(verify((root/'drivencheck_02.txt').read_text())),3)
        for changed in [text.replace('max_age_us=102','max_age_us=1001',1),
                        text.replace('scans=199','scans=0',1),
                        text.replace('gates_disabled=1','gates_disabled=0',1),
                        (root/'drivenadc_03.txt').read_text()]:
            with self.assertRaises((ValueError,RuntimeError)): verify(changed,real=True)

    def test_retained_disabled_timer_trials(self):
        text=(Path(__file__).resolve().parents[1]/'captures/drivencheck_01.txt').read_text()
        self.assertEqual(len(verify(text)),3)
        for changed in [text.replace('commands=24','commands=0',1),
                        text.replace('reason=2','reason=4',1),
                        text.replace('poststop_refused=1','poststop_refused=0',1),
                        text.split('FINALOFF')[0]]:
            with self.assertRaises((ValueError,RuntimeError)): verify(changed)
