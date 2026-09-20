import unittest
from pathlib import Path
from drv_boundary_pairs import pairs, report


class BoundaryPairsTests(unittest.TestCase):
    def rows(self):
        return [dict(us=(i+1)*500, step=i%6+1, reference_interval_ticks=1000)
                for i in range(24)]

    def test_displacement_identity_and_excluded_baseline(self):
        rows=self.rows()
        rows[15]['reference_interval_ticks'] += 200
        rows[16]['reference_interval_ticks'] -= 200
        r=pairs(rows, 100)[0]
        self.assertEqual(r['accepted_before'],91)
        self.assertEqual(r['step'],4)
        self.assertEqual(r['balanced_long_short_us'],100)
        self.assertEqual(r['pair_residual_us'],0)

    def test_uniform_sector_asymmetry_is_not_displacement(self):
        rows=self.rows()
        for r in rows:
            r['reference_interval_ticks'] += 20*r['step']
        self.assertTrue(all(r['balanced_long_short_us']==0 for r in pairs(rows,24)))

    def test_reject_gap_invalid_interval_and_short_baseline(self):
        rows=self.rows(); rows[9]['step']=6
        with self.assertRaises(ValueError): pairs(rows,24)
        rows=self.rows(); rows[9]['reference_interval_ticks']=0
        with self.assertRaises(ValueError): pairs(rows,24)
        with self.assertRaises(ValueError): pairs(self.rows(),23)
        self.assertEqual(pairs(self.rows()[:12],12),[])

    def test_retained_pass_and_failure_both_have_pairs(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,identity,amplitude in [
            ('carrier20_486_start61_hold70_10s_01.txt',393,67),
            ('carrier20_485_start61_reentry69_30s_02.txt',56018,56.25),
        ]:
            text=(root/name).read_text()
            r=report(text)
            self.assertEqual(r['largest_pairs'][0]['accepted_before'],identity)
            self.assertEqual(r['largest_pairs'][0]['balanced_long_short_us'],amplitude)
            self.assertEqual(r['outputs_off_verified'],1)
            self.assertFalse(r['population_rate_estimated'])
