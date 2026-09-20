import unittest
from pathlib import Path
from drv_driven_run import records,seed_window
from drv_driven_handoff import verify,verify_seed_timing

class TimingRestart(unittest.TestCase):
    def setUp(self):
        self.original=(Path(__file__).resolve().parents[1]/'captures/cycle360_553_start61_reentry71_30s_03.txt').read_text()
        self.marker='\nSEEDTIMING discarded_fault=3 max_restarts=1 fresh_intervals=12 original_deadline=1 experimental=1\n'
        self.text=self.original.replace('DRIVENSEEDRESTART count=0 anchor_epoch=0',
                                       'DRIVENSEEDRESTART count=1 anchor_epoch=2')+self.marker
        self.text+='\nDRIVENIRQBUDGET limit_us=50 overruns=0 immediate_stop=1 before_handoff=1 excludes_release=1\n'
        self.rows=records(self.original,'DI85',7)
    def test_selects_only_new_intervals_does_not_reclassify_failure(self):
        selected=seed_window(self.text,self.rows,True)
        self.assertEqual([r[0] for r in selected],list(range(2,15)))
        self.assertEqual(2*(selected[-1][2]-selected[0][2])//12,1543)
        with self.assertRaises(ValueError): verify(self.text,30000,dropout=True,reentry=True)
    def test_requires_explicit_fixture_option(self):
        verify_seed_timing(self.text,True)
        verify_seed_timing(self.original,False)
        for text,required in [(self.text,False),(self.original,True),(self.text+self.marker,True)]:
            with self.assertRaises(ValueError): verify_seed_timing(text,required)
        for text in [self.text.replace('overruns=0','overruns=1'),
                     self.text.replace('DRIVENIRQBUDGET','MISSINGBUDGET')]:
            with self.assertRaises(ValueError): verify_seed_timing(text,True)
    def test_bad_anchor_discarded_cause_and_duplicate_metadata_refuse(self):
        for text in [self.text.replace('anchor_epoch=2','anchor_epoch=3'),
                     self.text.replace('discarded_fault=3','discarded_fault=0'),
                     self.text.replace('discarded_fault=3','discarded_fault=4'),
                     self.text+self.marker,
                     self.original+self.marker]:
            with self.assertRaises(ValueError): seed_window(text,self.rows,True)
    def test_interval_must_corroborate_and_window_must_be_original(self):
        bad=list(self.rows);r=list(bad[2]);r[4]=1500;bad[2]=r
        with self.assertRaises(ValueError): seed_window(self.text,bad,True)
        late=[tuple(v+20000 if i in (2,3) else v for i,v in enumerate(row)) for row in self.rows]
        with self.assertRaises(ValueError): seed_window(self.text,late,True)
