import unittest
from pathlib import Path
from drv_baseline_modes import verify
from test_drv_driven_run import row
from test_drv_capture import OffReadbackTests


def fixture():
    lines=['BASEMODE complete=1 fault=0 same_wake=1 gates_off=1 n_target=128 period_us=201 first_us=1 stationary_verified=0 offsets_applied=0 dma_polled=1']
    for mode,elapsed in enumerate([14000,25600,14000]):
        lines.append(f'BASEMODEPART mode={mode} elapsed_us={elapsed}')
        for ch in [4,1,0,6,13]:
            value=2048+mode;n=128;total=n*value;squares=n*value*value
            lines.append(row('BM85',[mode,ch,n,total&65535,total>>16,
                                   squares&65535,(squares>>16)&65535,squares>>32,0,value,value]))
    return '\n'.join(lines+['BASEMODE END',OffReadbackTests.GOOD.decode()])


class BaselineModesTests(unittest.TestCase):
    def test_actual_settling_trials_and_retained_early_refusal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        with self.assertRaises(ValueError):verify((root/'basesettle_1ms_01.txt').read_text(),1)
        for ms,totals in [(1,[4.109375,3.44140625,2.875]),
                          (20,[-1.08984375,0.6953125,-1.05859375])]:
            for n,total in enumerate(totals,1):
                r=verify((root/f'basesettle_fix_{ms}ms_0{n}.txt').read_text(),ms)
                self.assertAlmostEqual(sum(r['dma_minus_sw_midpoint_counts'][:3]),total)
                self.assertEqual(r['settling']['fault_low_seen'],1)
                self.assertIsNone(r['amps'])

    def test_settling_provenance_and_requested_mode(self):
        for ms in [1,20]:
            marker=f'BASESETTLE target_us={ms*1000} measured_us={ms*1000+1} timer_sampled=1\n'
            r=verify(marker+fixture(),ms)
            self.assertEqual(r['settling']['measured_us'],ms*1000+1)
            for bad in [marker+marker+fixture(),fixture(),
                        marker.replace(f'measured_us={ms*1000+1}', 'measured_us=999')+fixture()]:
                with self.assertRaises(ValueError):verify(bad,ms)
        with self.assertRaises(ValueError):verify(marker+fixture(),1)

    def test_actual_three_idle_comparisons_do_not_imply_calibration(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for i,total in [(1,2.0625),(2,-2.09375),(3,1.1796875)]:
            r=verify((root/f'basemode_compare_0{i}.txt').read_text())
            self.assertAlmostEqual(sum(r['dma_minus_sw_midpoint_counts'][:3]),total)
            self.assertIsNone(r['amps'])
            self.assertTrue(r['outputs_off_verified'])

    def test_linear_change_is_not_claimed_as_mode_bias_or_current(self):
        r=verify(fixture())
        self.assertEqual(r['dma_minus_sw_midpoint_counts'],[0]*5)
        self.assertEqual(r['sw_after_minus_before_counts'],[2]*5)
        self.assertIsNone(r['amps']);self.assertFalse(r['drift_correction_proven'])

    def test_failure_metadata_timing_count_and_safing_refused(self):
        text=fixture()
        for bad in [text.replace('complete=1','complete=0'),text.replace('same_wake=1','same_wake=0'),
                    text.replace('elapsed_us=25600','elapsed_us=100'),
                    text.replace('BM85 ','WRONG ',1),text.split('BASEMODE END')[0]]:
            with self.subTest(bad=bad),self.assertRaises((ValueError,RuntimeError)):verify(bad)
