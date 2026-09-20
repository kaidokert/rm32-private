import unittest
from drv_phase_audit import audit


def record(visit,step,bracket):
    return dict(signal='late_on',complete=True,edges=1,supported=1,
                brackets=bracket,visit=visit,step=step,start_us=visit*3333)


class PhaseAudit(unittest.TestCase):
    def test_polarity(self):
        report=audit([record(i,i+1,f'1000:1200:{(i+1)&1}') for i in range(6)])
        self.assertTrue(all(r['expected_direction']==1 for r in report['steps']))
        self.assertFalse(report['qualified'])
    def test_opposite(self):
        report=audit([record(0,1,'1000:1200:0')])
        self.assertEqual(report['steps'][0]['opposite_direction'],1)
    def test_period_bounds_and_missing_visit(self):
        report=audit([record(0,1,'1000:1200:1'),record(6,1,'1000:1200:1'),record(18,1,'1000:1200:1')])
        periods=report['steps'][0]['same_step_periods']
        self.assertEqual(len(periods),1)
        self.assertEqual(periods[0]['period_us'],[19798,20198])


if __name__=='__main__':unittest.main()
