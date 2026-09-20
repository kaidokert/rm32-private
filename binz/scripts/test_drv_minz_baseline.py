import unittest
from pathlib import Path
import json
import tempfile
import zipfile
from minz_historical_baseline import summarize,verify_archive


class HistoricalBaselineTests(unittest.TestCase):
    def test_retained_historical_metrics_reproduce_without_live_minz(self):
        root=Path(__file__).resolve().parents[1]/'captures'/'reference'
        result=verify_archive(root/'minz_falcon_historical_20260913.zip',root/'minz_20260912.zip')
        self.assertTrue(result['metrics_reproduced'])
        self.assertFalse(result['capture_firmware_provenance_verified'])
        self.assertFalse(result['parity_proven'])
        self.assertEqual(result['metrics']['lockmap_a9.bin']['missing_qzc_windows'],1)

    def test_corrupt_payload_or_changed_metrics_refused(self):
        root=Path(__file__).resolve().parents[1]/'captures'/'reference'
        with zipfile.ZipFile(root/'minz_falcon_historical_20260913.zip') as z:
            original={name:z.read(name) for name in z.namelist()}
        with tempfile.TemporaryDirectory() as tmp:
            for case in ['payload','metrics','missing']:
                data=dict(original)
                if case=='payload':
                    data['captures/lockmap_a9.bin']+=b'!'
                else:
                    m=json.loads(data['manifest.json'])
                    if case=='metrics': m['metrics']['lockmap_a9.bin']['qzc_percent']=100
                    else: m['metrics']={}
                    data['manifest.json']=json.dumps(m).encode()
                path=Path(tmp)/(case+'.zip')
                with zipfile.ZipFile(path,'w') as z:
                    for name,raw in data.items(): z.writestr(name,raw)
                with self.assertRaises(ValueError):
                    verify_archive(path,root/'minz_20260912.zip')

    def frames(self):
        return [dict(seq=i%256,sector=i%6,len_us=800,qzc_off_us=400,i_avg=10) for i in range(300)]

    def test_wrap_and_independent_window_denominator(self):
        frames=self.frames()
        frames[20]['qzc_off_us']=65535
        frames[21]['qzc_off_us']=65535
        result=summarize(frames,float)
        self.assertEqual(result['sequence_discontinuities'],0)
        self.assertEqual(result['missing_qzc_windows'],2)
        self.assertEqual(result['longest_observed_missing_run'],2)
        self.assertAlmostEqual(result['qzc_percent'],100*298/300)
        self.assertFalse(result['parity_proven'])

    def test_gap_does_not_join_missing_runs(self):
        frames=self.frames()
        for i in range(20,24): frames[i]['qzc_off_us']=65535
        del frames[22]
        result=summarize(frames,float)
        self.assertEqual(result['sequence_discontinuities'],1)
        self.assertEqual(result['longest_observed_missing_run'],2)

    def test_time_weighted_current_and_insufficient_data(self):
        frames=self.frames()
        for f in frames[150:]: f.update(len_us=1600,i_avg=20)
        result=summarize(frames,float)
        self.assertEqual(result['nominal_window_mean_current_ma'],15)
        self.assertAlmostEqual(result['nominal_time_weighted_current_ma'],50/3)
        with self.assertRaises(ValueError): summarize(frames[:99],float)
