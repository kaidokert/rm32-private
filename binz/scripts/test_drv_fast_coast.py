import unittest
from unittest.mock import patch
from pathlib import Path
from drv_fast_coast import verify


class FastCoastTests(unittest.TestCase):
    def test_old_capture_requires_old_mode(self):
        text=(Path(__file__).resolve().parents[1]/'captures/dmaguard_626_ramp220_30s.txt').read_text()
        self.assertIsNone(verify(text))
        with self.assertRaises(ValueError):verify(text,True)

    def test_measured_not_requested_cadence(self):
        text='\n'.join(f'COASTCOMP row={i} a_us=70 b_us=82 c_us=94 from_scan_start=1' for i in range(32))
        for gap,good in [(124,False),(125,True),(150,True),(151,False)]:
            rows=[dict(elapsed_us=(i+1)*gap) for i in range(500)]
            with patch('drv_fast_coast.parse_dump',return_value=([],0,rows,8000,1662,2)):
                if good:self.assertEqual(verify(text,True)['max_gap_us'],gap)
                else:
                    with self.assertRaises(ValueError):verify(text,True)
                with self.assertRaises(ValueError):verify(text,False)
                with self.assertRaises(ValueError):verify(text.replace('c_us=94','c_us=125'),True)
                with self.assertRaises(ValueError):verify(text.replace('row=0 ','row=1 ',1),True)


if __name__=='__main__':unittest.main()
