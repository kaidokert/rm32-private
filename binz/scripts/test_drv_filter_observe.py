import unittest
from drv_filter_observe import verify
class FilterObserveTests(unittest.TestCase):
    ROW='FILTEROBS samples=10 captures=8 overcapture=2 lag_min_ticks=4 lag_max_ticks=100 active=0 authority=0 latest_capture=1 settling_excluded=0'
    def test_valid(self):self.assertEqual(verify(self.ROW)['captures'],8)
    def test_raw_modulo(self):
        rows='\n'.join(f'FILTERRAW index={i} ready=1 captured=1 over=0 ccr=199 counter=2 period_us=201 age_modulo_only=1' for i in range(2))
        text=self.ROW+'\n'+rows
        self.assertEqual(verify(text,require_raw=True)['raw_prefix'][0]['age_modulo_us'],4)
        for old,new in [('ready=1','ready=0'),('counter=2','counter=201'),('age_modulo_only=1','age_modulo_only=0')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1),require_raw=True)
        with self.assertRaises(ValueError):verify(self.ROW,require_raw=True)
    def test_config(self):
        text='FILTERCONFIG code=12 ckd=2 observer_only=1\n'+self.ROW
        self.assertEqual(verify(text,12)['captures'],8)
        with self.assertRaises(ValueError):verify(text,15)
        with self.assertRaises(ValueError):verify(self.ROW,12)
    def test_detail(self):
        text=self.ROW+'\n'+'\n'.join(f'FILTERSECTOR step={i} samples={10 if i==1 else 0} captures={8 if i==1 else 0}' for i in range(1,7))
        text+='\n'+'\n'.join(f'FILTERMISS index={i} step=1 rising=1 raw_after=1 counter=1000 arm_age=700 pwm=42' for i in range(2))
        self.assertEqual(len(verify(text,require_detail=True)['miss_prefix']),2)
        for old,new in [('step=6 samples=0','step=6 samples=1'),('pwm=42','pwm=2666'),('raw_after=1','raw_after=2'),('index=1','index=2')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1),require_detail=True)
        with self.assertRaises(ValueError):verify(self.ROW,require_detail=True)
    def test_reject(self):
        for old,new in [('captures=8','captures=11'),('overcapture=2','overcapture=9'),('active=0','active=1'),('authority=0','authority=1'),('lag_min_ticks=4','lag_min_ticks=101')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(self.ROW.replace(old,new))
        with self.assertRaises(ValueError):verify(self.ROW+'\n'+self.ROW)
    def test_empty(self):
        self.assertEqual(verify(self.ROW.replace('samples=10','samples=0').replace('captures=8','captures=0').replace('overcapture=2','overcapture=0').replace('lag_min_ticks=4','lag_min_ticks=4294967295').replace('lag_max_ticks=100','lag_max_ticks=0'))['samples'],0)
