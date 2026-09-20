"""Real retained captures; failed campaigns must not become scaling evidence."""
import unittest
from pathlib import Path
from drv_architecture_report import report, accounting


class ArchitectureTests(unittest.TestCase):
    root=Path(__file__).resolve().parents[1]/'captures'
    def test_synthetic_exclusive_partition_not_double_counted(self):
        from test_drv_cpu_meter import fixture
        r=accounting(fixture())
        self.assertEqual(r['irq_union_percent'],50)
        self.assertEqual(r['accounting_kind'],'per_vector')
        rows=r['per_vector_exclusive_cost']
        self.assertEqual(sum(x['exclusive_us'] for x in rows),50)
        self.assertEqual(rows[1]['name'],'comparator')
        self.assertEqual(rows[1]['mean_exclusive_us_per_call'],30)
        self.assertFalse(r['probe_cost_measured'])
        with self.assertRaises(ValueError):accounting(fixture().replace('fault=0','fault=1'))

    def test_single_core_recovery_counts_reset_per_segment(self):
        r=report((self.root/'singlecore_reentry62_30s_01.txt').read_text(),reentry=True)
        self.assertEqual(r['comparator_paths']['accepted'],47028)
        self.assertEqual(r['comparator_paths']['dispatched'],498017)
        self.assertAlmostEqual(r['irq_union_percent'],62.7945536,places=5)
    def test_real_instrumented_hold_counts(self):
        r=report((self.root/'comppaths_hold62_01.txt').read_text())
        self.assertEqual(r['comparator_paths'],dict(no_gate=0,closed=21526,
            open_no_accept=106905,accepted=16767,stopped_or_unknown=0,dispatched=145198))
        self.assertAlmostEqual(r['irq_union_percent'],70.1257069,places=5)
        self.assertIsNone(r['removable_irq_fraction'])

        self.assertAlmostEqual(r['closed_gate_fraction_of_dispatched'],21526/145198)
        self.assertAlmostEqual(r['open_no_accept_fraction_of_dispatched'],106905/145198)
        self.assertFalse(r['decision_fractions_are_cpu_fractions'])

    def test_real_qualified_recovery_counts_and_unknowns(self):
        text=(self.root/'inline24_reentry62_30s_01.txt').read_text()
        r=report(text,reentry=True)
        self.assertEqual(r['nonaccepting_comp_calls'],435097-46854)
        self.assertAlmostEqual(r['irq_union_percent'],67.37006,places=4)
        self.assertAlmostEqual(r['comp_calls_per_com'],435097/46854)
        self.assertIsNone(r['removable_irq_fraction'])
        self.assertIsNone(r['blank_gate_calls'])
        self.assertIsNone(r['closed_gate_fraction_of_dispatched'])
        self.assertIsNone(r['open_no_accept_fraction_of_dispatched'])
        self.assertFalse(r['throttle_scaling_inferred'])
        self.assertFalse(r['maxima_are_additive'])

    def test_failed_recovery_and_truncated_safing_refused(self):
        with self.assertRaises((ValueError,RuntimeError)):
            report((self.root/'roles_reentry60_30s_01.txt').read_text(),reentry=True)
        text=(self.root/'inline24_hold62_01.txt').read_text()
        with self.assertRaises((ValueError,RuntimeError)):
            report(text[:text.rfind('COAST END')])

    def test_synthetic_path_metadata_accounting_on_real_capture(self):
        # Synthetic metadata tests parser/accounting only, not measured paths.
        text=(self.root/'inline24_reentry62_30s_01.txt').read_text()
        row=('\nCOMPPATH scope=dispatched first_actual_count=1 no_gate=0 '
             'closed=300000 open_no_accept=88243 accepted=46854 '
             'stopped_or_unknown=0 active=0\n')
        r=report(text+row,reentry=True)
        self.assertEqual(r['blank_gate_calls'],300000)
        self.assertIsNone(r['persistence_reject_calls'])
        self.assertIsNone(r['removable_irq_fraction'])
        for bad in [row.replace('accepted=46854','accepted=46853'),
                    row.replace('closed=300000','closed=300001')]:
            with self.assertRaises(ValueError):
                report(text+bad,reentry=True)


if __name__=='__main__':unittest.main()
