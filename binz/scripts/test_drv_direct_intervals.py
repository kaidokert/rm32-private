from copy import deepcopy
from pathlib import Path
import unittest
from drv_direct_intervals import report,join_rows,same_sector_changes
from drv_qualification_direct import decode_campaign
from drv_accepted_events import decode_windows


class DirectIntervalTests(unittest.TestCase):
    def test_v2_actual_rejection_positions_survive_join_without_causal_claim(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        failed=report((root/'reject_518_start61_hold70_10s_01.txt').read_text())
        row=next(r for r in failed['joined'] if r['accepted_before']==431)
        self.assertEqual(row['rejected_read_indices'],[0,1,11])
        self.assertEqual(row['reference_interval_us'],539.5)
        self.assertEqual(failed['partial']['rejected_read_indices'],[0])
        self.assertTrue(failed['outputs_off_verified'])
        self.assertFalse(failed['causal_mechanism_proven'])
        passed=report((root/'reject_517_start61_reentry69_30s_01.txt').read_text())
        self.assertEqual(passed['outcome'],'powered_window_complete')
        self.assertTrue(any(any(i>0 for i in r['rejected_read_indices']) for r in passed['joined']))
        self.assertFalse(passed['persistence_failures_counted'])

    def text(self,failed=True):
        name='direct_512_start61_hold70_10s_01.txt' if failed else 'direct_511_start61_reentry69_30s_01.txt'
        return (Path(__file__).resolve().parents[1]/'captures'/name).read_text()

    def test_fault_boundary_and_partial_are_distinct(self):
        r=report(self.text())
        pair=r['largest_covered_pairs'][0]
        self.assertEqual(pair['pair']['accepted_before'],873)
        self.assertEqual((pair['left']['open'],pair['right']['open']),(11,5))
        self.assertEqual(pair['left']['first_to_last_open_us'],302.5)
        self.assertEqual(r['partial']['accepted_before'],879)
        self.assertEqual(len(r['joined']),16)
        self.assertTrue(r['outputs_off_verified'])
        self.assertFalse(r['persistence_failures_counted'])

    def test_passing_run_also_has_multiple_open_visits(self):
        r=report(self.text(False))
        self.assertEqual(r['outcome'],'powered_window_complete')
        self.assertGreater(min(row['open'] for row in r['joined']),1)

    def test_cross_stream_identity_sector_and_coverage_refuse(self):
        t=self.text();d=decode_campaign(t,True);w=decode_windows(t)
        for kind in ('total','sector','coverage'):
            bad=deepcopy(d)
            if kind=='total':bad['final_accepted']+=1
            elif kind=='sector':bad['rows'][0]['step']=bad['rows'][0]['step']%6+1
            else:bad['rows'][0]['accepted_before']=0
            with self.assertRaises(ValueError):join_rows(bad,w)

    def test_corrupt_wire_refuses(self):
        with self.assertRaises(ValueError):report(self.text().replace('QD85 ','QD85 !',1))

    def test_previous_same_sector_localizes_extension_without_filling_gaps(self):
        r=report(self.text())
        change=next(x for x in r['same_sector_changes'] if x['accepted_before']==873)
        self.assertEqual(change['previous_before'],867)
        self.assertEqual((change['open_change'],change['closed_change']),(6,-1))
        self.assertEqual(change['first_open_change_us'],-20.5)
        self.assertEqual((change['last_open_change_us'],change['reference_interval_change_us']),(111,111))
        missing=[x for x in r['joined'] if x['accepted_before']!=867]
        self.assertNotIn(873,[x['accepted_before'] for x in same_sector_changes(missing)])
        bad=deepcopy(r['joined']);bad[6]['step']=bad[6]['step']%6+1
        with self.assertRaises(ValueError):same_sector_changes(bad)
