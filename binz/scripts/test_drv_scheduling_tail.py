import unittest
from drv_scheduling_tail import decode
from test_drv_driven_run import row


def fixture(events=None,omitted=0,fault=0):
    if events is None:events=[(1,2,0,2),(3,17,0,1),(7,2,1,1),(10,2,2,0)]
    n=len(events);count=n+omitted
    header=(f'SCHEDTAIL count={count} retained={n} omitted={omitted} elapsed_us={events[-1][0]} '
            f'max_gap_us=4 started=1 active=0 fault={fault} capacity=64 row_words=4 '
            'first_guard_origin=1 shared_cpu_clock=1 physical_edges=0 blackout_watchdog=0')
    lines=[header]
    for at,stack,kind,ident in events:
        packed=stack|(kind<<18)|(ident<<20)
        lines.append(row('ST85',[at&65535,at>>16,packed&65535,packed>>16]))
    return '\n'.join(lines)


class SchedulingTailTests(unittest.TestCase):
    def test_hardware_probe_rejected_and_restored_baseline_passes(self):
        from pathlib import Path
        from drv_cpu_check import verify
        from drv_capture import verify_off
        root=Path(__file__).resolve().parents[1]/'captures'
        failed=(root/'schedtail_cpu02.txt').read_text()
        verify_off(failed[failed.rfind('FINALOFF'):].encode())
        self.assertIn('mode=2 n=256 sum_us=6656 max_us=26 fault=0',failed)
        with self.assertRaises(ValueError):verify(failed,union=True,scheduling_tail=True)
        restored=verify((root/'schedtail_restorecpu01.txt').read_text(),union=True)
        self.assertEqual([r['max_pair_us'] for r in restored],[2,7,10])

    def test_cpu_epoch_counts_and_required_disabled_marker(self):
        from drv_scheduling_tail import verify_cpu
        from drv_cpu_check import verify as check
        cpu=('CPUUNION elapsed_us=10 started=1 active=0 fault=0 max_gap_us=4 '
             'max_depth=2 stopped_depth=1 contexts=2 first_guard_origin=1 '
             'exception_overhead_separate=0 probe_cost_measured=0 foreground_is_idle=0\n')
        cpu+=row('CU85',[0,0,0,1,0])+'\n'+row('CU85',[1,2,0,9,0])
        good=fixture()+'\n'+cpu
        self.assertTrue(verify_cpu(good,True)['valid'])
        for bad in [fixture(),good.replace('stopped_depth=1','stopped_depth=0'),
                    good.replace('elapsed_us=10','elapsed_us=11',1)]:
            with self.assertRaises(ValueError):verify_cpu(bad,True)
        from unittest.mock import patch
        checks='FINALOFF\nCPUCHECKTYPE union=1\n'+ '\n'.join(
            f'CPUCHECK mode={i} n=256 sum_us=256 max_us=1 fault=0 software_pairs_only=1 gate_authority=0 disabled=1' for i in range(3))
        with patch('drv_cpu_check.verify_off'):
            with self.assertRaises(ValueError):check(checks,union=True,scheduling_tail=True)
            marker='\nCPUCHECKTAIL enabled=1 nested_exercised=1 fault_checked=1'
            self.assertEqual(len(check(checks+marker,union=True,scheduling_tail=True)),3)
            with self.assertRaises(ValueError):check(checks+marker+marker,scheduling_tail=True)

    def test_nested_stop_and_strict_provenance(self):
        good=fixture();r=decode(good,True)
        self.assertEqual(r['rows'][1]['active_stack'],[2,1])
        self.assertEqual(r['rows'][-1]['active_stack'],[2])
        self.assertFalse(r['physical_edge_latency'])
        self.assertIsNone(decode(''))
        for bad in ['',good+'\n'+good,good.replace('physical_edges=0','physical_edges=1'),
                    good.replace('retained=4','retained=3'),good.replace('active=0','active=1'),
                    good+'\n'+good.splitlines()[-1]]:
            with self.assertRaises(ValueError):decode(bad,True)

    def test_bad_transitions_and_missing_stop(self):
        for events in [[(1,2,0,1)],[(1,2,0,2),(3,3,1,2)],
                       [(1,18,0,2)],[(2,2,0,2),(1,0,1,2)],
                       [(1,2,0,2),(2,2,2,0),(3,0,1,2)]]:
            with self.assertRaises(ValueError):decode(fixture(events),True)

    def test_truncated_partial_handler_and_fault_not_a_pass(self):
        events=[]
        for n in range(31):events.extend([(n*2+1,2,1,1),(n*2+2,17,0,1)])
        events.extend([(63,2,1,1),(64,2,2,0)])
        r=decode(fixture(events,omitted=100),True)
        self.assertEqual(r['omitted'],100)
        self.assertFalse(r['complete_history'])
        bad=fixture([(1,2,0,2)],fault=4)
        self.assertFalse(decode(bad)['valid'])
        with self.assertRaises(ValueError):decode(bad,True)


if __name__=='__main__':unittest.main()
