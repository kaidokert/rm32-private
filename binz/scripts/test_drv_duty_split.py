import unittest
from drv_duty_split import verify

class DutySplitTests(unittest.TestCase):
    def test_independent_and_inherited(self):
        self.assertIsNone(verify('old capture'))
        for duty in [40,62,70,100]:
            row=f'DUTYSPLIT acquisition=62 bemf={duty} units=tenths_percent segment_fixed=1'
            self.assertEqual(verify(row,acquisition=62,bemf=duty)['bemf'],duty)
            with self.assertRaises(ValueError):verify(row,acquisition=61,bemf=duty)
            with self.assertRaises(ValueError):verify(row+'\n'+row,bemf=duty)
    def test_missing_out_of_range_and_mismatch(self):
        for row in ['', 'DUTYSPLIT acquisition=63 bemf=70 units=tenths_percent segment_fixed=1',
                    'DUTYSPLIT acquisition=62 bemf=101 units=tenths_percent segment_fixed=1',
                    'DUTYSPLIT acquisition=62 bemf=62 units=tenths_percent segment_fixed=1']:
            with self.assertRaises(ValueError):verify(row,bemf=70)
