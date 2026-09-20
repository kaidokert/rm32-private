import base64
from pathlib import Path
import struct
import unittest
import zlib
from drv_zero_check import decode,normalized_drift


def fixture():
    text='ZEROCHK result=0 elapsed_us=100000 disabled=1 same_powered_reader=1 applied_calibration=0\n'
    for ch in [4,1,0,6,13]:
        total=2048*512;squares=2048**2*512
        w=[ch,512,total&65535,total>>16]+[(squares>>(16*i))&65535 for i in range(4)]+[2048,2048]
        raw=struct.pack('<10H',*w)
        text+='Z85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'
    real=(Path(__file__).resolve().parents[1]/'captures/reentry_timeline_arm_01.txt').read_text()
    return text+real[real.rfind('OUT:'):]


class ZeroCheckTests(unittest.TestCase):
    def test_voltage_normalization_distinguishes_adc_scale_from_offset_drift(self):
        from copy import deepcopy
        data=decode(fixture())
        data['second_channels']=deepcopy(data['channels'])
        # A common ADC scale change leaves voltage-domain offsets unchanged.
        for row in data['second_channels']: row['mean_raw']*=1.01
        result=normalized_drift(data,1662)
        for delta in result['csa_delta_mv']: self.assertAlmostEqual(delta,0)
        self.assertFalse(result['powered_current_qualified'])
        data['second_channels'][0]['mean_raw']+=1
        result=normalized_drift(data,1662)
        expected=result['windows'][1]['vdda_mv']/4096*1000/70
        self.assertAlmostEqual(result['summed_output_drift_equivalent_ma_at_70mv_per_a'],expected)

    def test_voltage_normalization_requires_valid_reference(self):
        data=decode(fixture())
        self.assertIsNone(normalized_drift(data,1662)['summed_output_drift_equivalent_ma_at_70mv_per_a'])
        for vcal in [0,4096,True,1.2]:
            with self.assertRaises(ValueError): normalized_drift(data,vcal)
        data['channels'][4]['mean_raw']=0
        with self.assertRaises(ValueError): normalized_drift(data,1662)

    def test_same_wake_pair_requires_both_complete_windows(self):
        text=fixture().replace('applied_calibration=0','applied_calibration=0 windows=2 same_wake=1')
        extra='\n'.join(l.replace('Z85 ','ZB85 ') for l in text.splitlines() if l.startswith('Z85 '))+'\n'
        text=text.replace('OUT:',extra+'OUT:')
        self.assertEqual(decode(text)['same_wake_delta_raw'],[0]*5)
        with self.assertRaises(ValueError): decode(text.replace('ZB85 ','MISSING '))
    def test_real_capture_and_failed_immediate_shutdown_readback(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        data=decode((root/'zero_guarded_02.txt').read_text())
        self.assertEqual(data['channels'][0]['sum'],1051377)
        self.assertAlmostEqual(data['channels'][0]['mean_raw'],2053.470703125)
        with self.assertRaises(ValueError): decode((root/'zero_guarded_01.txt').read_text())
    def test_exact_moments_and_zero_variance(self):
        rows=decode(fixture())['channels']
        self.assertTrue(all(r['mean_raw']==2048 and r['sd_raw']==0 for r in rows))

    def test_missing_corrupt_failed_or_unverified_measurement_refused(self):
        text=fixture();line=next(l for l in text.splitlines() if l.startswith('Z85 '))
        for changed in [text.replace(line,''),text.replace(line,line[:-1]+'!'),
                        text.replace('result=0','result=3'),text.replace('elapsed_us=100000','elapsed_us=300000'),
                        text.replace('OUT:','MISSING:'),text+'\n'+text.splitlines()[0]]:
            with self.assertRaises((ValueError,RuntimeError)): decode(changed)
