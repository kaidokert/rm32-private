import unittest
from pathlib import Path
from drv_cpu_meter import decode,decode_roots
from test_drv_driven_run import row


def fixture():
    header = ('CPUMETER elapsed_us=100 started=1 active=0 fault=0 max_gap_us=50 '
              'max_depth=2 stopped_depth=0 contexts=7 first_guard_origin=1 '
              'exception_overhead_separate=0 probe_cost_measured=0 foreground_is_idle=0')
    return '\n'.join([header] + [row('CPU85', [i, 0 if i == 0 else 1, 0, t, 0])
                                for i, t in enumerate([50, 10, 30, 10, 0, 0, 0])])


class CpuMeterTests(unittest.TestCase):
    def test_root_attribution_distinct_and_partitions_union(self):
        header=fixture().splitlines()[0].replace('CPUMETER','CPUUNION').replace('contexts=7','contexts=2')
        base='\n'.join([header,row('CU85',[0,0,0,50,0]),row('CU85',[1,6,0,50,0])])
        root='\nCPUROOT contexts=6 includes_nested=1 exclusive=0 extra_clock_reads=0\n'
        root+='\n'.join(row('CR85',[i,t,0]) for i,t in enumerate([0,40,10,0,0,0],1))
        r=decode_roots(base+root,decode(base))
        self.assertEqual(r['partition_us'],[0,40,10,0,0,0])
        self.assertFalse(r['exclusive'])
        self.assertIsNone(decode_roots(base,decode(base)))
        for bad in [root.replace('exclusive=0','exclusive=1'),root+root,
                    root+'\n'+row('CR85',[1,0,0]),
                    root.replace(row('CR85',[2,40,0]),row('CR85',[2,39,0]))]:
            with self.assertRaises(ValueError):decode_roots(base+bad,decode(base))
        with self.assertRaises(ValueError):decode_roots(fixture()+root,decode(fixture()))

    def test_instrumented_recovery_and_staged_range(self):
        from drv_driven_handoff import verify
        from drv_sustained_report import summarize
        root = Path(__file__).resolve().parents[1] / 'captures'
        text = (root / 'cpu_union_reentry45_01.txt').read_text()
        result = verify(text, 10000, dropout=True, reentry=True)
        self.assertTrue(result['powered_recovery_verified'])
        self.assertEqual(int(result['powered']['original_end_elapsed_us']) -
                         int(result['powered']['final_elapsed_us']), 159)
        cpu = decode(text)
        self.assertTrue(cpu['valid'])
        self.assertEqual(cpu['partition_us'], [3792319, 4195593])
        for duty, irq_us in [(46, 5250266), (48, 5234063), (50, 5290988)]:
            text = (root / f'cpu_union_range{duty}_01.txt').read_text()
            result = verify(text, 10000)
            self.assertTrue(result['powered_handoff_window_verified'])
            self.assertEqual(result['acquisition']['duty_tenths'], duty)
            self.assertEqual(result['powered']['run_cycle_min_us'], '3333')
            self.assertTrue(decode(text)['valid'])
            self.assertEqual(decode(text)['partition_us'][1], irq_us)
        stopped = (root / 'cpu_union46_01.txt').read_text()
        self.assertEqual(summarize(stopped)['powered_reason'], '12')
        self.assertTrue(decode(stopped)['valid'])
        with self.assertRaises(ValueError):
            verify(stopped, 10000)

    def test_real_one_and_ten_second_holds(self):
        from drv_driven_handoff import verify
        root = Path(__file__).resolve().parents[1] / 'captures'
        for n, ms, elapsed, irq_us, calls in [(1, 1000, 1000008, 522718, 22375),
                                             (2, 10000, 10000010, 5278743, 224901)]:
            text = (root / f'cpu_union45_{n:02}.txt').read_text()
            self.assertTrue(verify(text, ms)['powered_handoff_window_verified'])
            result = decode(text)
            self.assertTrue(result['valid'])
            self.assertEqual(result['elapsed_us'], elapsed)
            self.assertEqual(result['partition_us'], [elapsed - irq_us, irq_us])
            self.assertEqual(result['calls'], [0, calls])
            self.assertIsNone(result['cpu_utilization_percent'])

    def test_union_is_distinct_and_mixed_protocol_refuses(self):
        header = fixture().splitlines()[0].replace('CPUMETER', 'CPUUNION').replace('contexts=7', 'contexts=2')
        text = '\n'.join([header, row('CU85', [0, 0, 0, 50, 0]), row('CU85', [1, 6, 0, 50, 0])])
        result = decode(text)
        self.assertEqual(result['kind'], 'irq_union')
        self.assertEqual(result['partition_us'], [50, 50])
        for bad in [text + '\n' + fixture(), text.replace('contexts=2', 'contexts=7'),
                    text + '\n' + row('CU85', [1, 6, 0, 50, 0])]:
            with self.assertRaises(ValueError):
                decode(bad)

    def test_partition_is_not_idle_or_utilization(self):
        result = decode(fixture())
        self.assertTrue(result['valid'])
        self.assertEqual(sum(result['partition_us']), 100)
        self.assertIsNone(result['cpu_utilization_percent'])
        self.assertFalse(result['foreground_is_idle'])

    def test_bad_records_or_provenance_refuse(self):
        text = fixture()
        for bad in [text + '\n' + text.splitlines()[0], text + '\n' + text.splitlines()[1],
                    '\n'.join(text.splitlines()[:-1]), text.replace('elapsed_us=100', 'elapsed_us=99'),
                    text.replace('active=0', 'active=1'), text.replace('started=1', 'started=0'),
                    text.replace('max_gap_us=50', 'max_gap_us=1001'),
                    text.replace('probe_cost_measured=0', 'probe_cost_measured=1'),
                    text.replace('stopped_depth=0', 'stopped_depth=3'),
                    text[:-1] + ('!' if text[-1] != '!' else '#')]:
            with self.assertRaises(ValueError):
                decode(bad)

    def test_visible_gap_is_invalid_evidence_not_clean_occupancy(self):
        result = decode(fixture().replace('fault=0', 'fault=1').replace('max_gap_us=50', 'max_gap_us=1001'))
        self.assertFalse(result['valid'])
        self.assertEqual(result['fault'], 1)


if __name__ == '__main__':
    unittest.main()
