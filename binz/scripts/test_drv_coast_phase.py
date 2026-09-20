import math
from pathlib import Path
import unittest
from drv_coast_phase import fits,analyze
from drv_driven_run import coast_origin
from test_drv_driven_run import row

class CoastPhaseTests(unittest.TestCase):
    def test_actual_measured_gap_and_phase_sweep_keep_authority_off(self):
        from drv_driven_run import verify
        root=Path(__file__).resolve().parents[1]
        baseline=(root/'captures/driven_coast_anchor_01.txt').read_text()
        self.assertEqual(coast_origin(baseline),(10,11))
        for name,phase,candidates in [('driven_phase30_01',30,2),('driven_phase60_01',60,10)]:
            r=verify((root/f'captures/{name}.txt').read_text())
            self.assertEqual(r['phase_shift_degrees'],phase)
            self.assertEqual(r['pwm_offline_candidate_sectors'],candidates)
            self.assertEqual(r['handoff_authority'],0)
    def test_clock_origin_bracket_must_be_valid_and_crc_protected(self):
        header='DRIVENCOAST fields=valid,delay_min_us,delay_max_us t17_brackets_coast_origin=1\n'
        self.assertEqual(coast_origin(header+row('CB85',[1,23,25])),(23,25))
        for values in [[0,23,25],[1,25,23],[1,0,21],[1,1000,1001]]:
            with self.assertRaises(ValueError): coast_origin(header+row('CB85',values))
    def test_measured_origin_enables_only_conditional_phase_projection(self):
        text=(Path(__file__).resolve().parents[1]/'captures/driven_du46_coast_01.txt').read_text()
        # Synthetic clock bracket on a real trace; tests math,not hardware provenance.
        text+='\nDRIVENCOAST fields=valid,delay_min_us,delay_max_us t17_brackets_coast_origin=1\n'+row('CB85',[1,20,22])+'\n'
        result=analyze(text,100)
        self.assertTrue(result['stop_to_coast_offset_measured'])
        self.assertGreater(len(result['directions'][0]['relative_phase_at_stop_grid_degrees']),0)
        self.assertFalse(result['phase_correction_authorized'])
    def test_known_forward_sine_and_wrong_direction(self):
        samples=[(t,t,p,math.sin(math.radians(37+120*p+200*360*t/1e6))<0)
                 for t in range(137,6000,173) for p in range(3)]
        self.assertIn((200,37),fits(samples,[200],range(360)))
        self.assertEqual(fits(samples,[200],range(360),-1),[])
    def test_interval_censoring_allows_crossing_inside_bracket(self):
        self.assertEqual(fits([(2400,2600,0,True)],[200],[0]),[(200,0)])
        self.assertEqual(fits([(200,300,0,True)],[200],[0]),[])
    def test_wide_intervals_are_not_silently_accepted(self):
        with self.assertRaises(ValueError): fits([(0,2500,0,True)],[200],[0])
        with self.assertRaises(ValueError): fits([],direction=0)
    def test_real_capture_fit_cannot_authorize_phase_correction(self):
        root=Path(__file__).resolve().parents[1]
        text=(root/'captures/driven_du46_coast_01.txt').read_text()
        result=analyze(text,100)
        self.assertGreater(result['directions'][0]['feasible_grid_points'],0)
        self.assertEqual(result['directions'][1]['feasible_grid_points'],0)
        self.assertFalse(result['stop_to_coast_offset_measured'])
        self.assertFalse(result['phase_correction_authorized'])
        with self.assertRaises(ValueError): analyze(text.replace('COASTCOMP row=0','MISSING row=0'),100)

if __name__=='__main__': unittest.main()
