import unittest
from pathlib import Path
from drv_timing_report import report, seed_stages


class SeedStagesTests(unittest.TestCase):
    def test_verified_recovery_stages(self):
        text=(Path(__file__).resolve().parents[1]/'captures/staticcomp_reentry66_30s_01.txt').read_text()
        r=report(text,reentry=True)['seed_stage_timing']
        self.assertEqual(list(r['brackets_us'].values()),[49,10,7,20,2,8])
        self.assertEqual(sum(r['brackets_us'].values()),r['arm_age_us'])
        self.assertFalse(r['exclusive_cost'])
        self.assertFalse(r['removable_delay_proven'])

    def test_missing_duplicate_reordered_and_unset_refuse(self):
        self.assertIsNone(seed_stages('legacy',192))
        good='SEEDLAT entry_ticks=98 reset_ticks=118 feedback_ticks=132 guard_ticks=172 reference_ticks=176 half_us=1\n'
        for bad,age in [(good+good,192),(good.replace('118','90'),192),
                        (good.replace('98','4294967295'),192),(good,170),
                        (good.replace('half_us=1','half_us=0'),192),('SEEDLAT\n',192)]:
            with self.assertRaises(ValueError):seed_stages(bad,age)
