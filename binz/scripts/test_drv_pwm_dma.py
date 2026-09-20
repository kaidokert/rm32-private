import unittest
from drv_pwm_dma import verify
from test_drv_driven_run import row

def fixture():
    result=[]
    for target in [192,320]:
        result.append(f'PWMDMA reason=0 target={target} n=64 elapsed_us=6420 adc_scans=60 flags=7 stopped=1 disabled=1 source_tim1_cnt=1 gate_authority=0')
        result.extend(row('PM85',[i,target+8,0]) for i in range(64));result.append('PWMDMA END')
    result.extend(['FINALOFF','OUT: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 TIM1:moe=0 ccrA=0 ccrB=0 ccrC=0',
        'IN: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 nflt=1'])
    return '\n'.join(result)

class PwmDmaTests(unittest.TestCase):
    def test_complete_two_targets(self): self.assertEqual(len(verify(fixture())),2)
    def test_request_latency_not_silently_widened(self):
        with self.assertRaises(ValueError): verify(fixture().replace(row('PM85',[1,200,0]),row('PM85',[1,225,0])))
    def test_missing_record(self):
        with self.assertRaises(ValueError): verify(fixture().replace(row('PM85',[1,200,0]),''))
    def test_finite_count_flags_off_and_provenance(self):
        for a,b in [('n=64','n=63'),('flags=7','flags=15'),('stopped=1','stopped=0'),('source_tim1_cnt=1','source_tim1_cnt=0'),('gate_authority=0','gate_authority=1')]:
            with self.subTest(a=a),self.assertRaises(ValueError): verify(fixture().replace(a,b))
    def test_no_adc_contention_or_missing_off(self):
        for a,b in [('adc_scans=60','adc_scans=0'),('FINALOFF','NO_FINAL')]:
            with self.assertRaises(ValueError): verify(fixture().replace(a,b))

if __name__=='__main__': unittest.main()
