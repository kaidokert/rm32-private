import unittest
from pathlib import Path
from drv_sparse_check import verify,PRELOADED,LIMITS
from test_drv_qualification_window import record

class SparseCheck(unittest.TestCase):
    def sample(self):
        # Reuse real off grammar, never pretend these synthetic costs were timed.
        old=Path('captures/qualsplit_scope01.txt').read_text().replace('\r','')
        final=old.split('FINALOFF\n')[1]
        rows='\n'.join(f'SPARSECHECK mode={i} passed=16 total=16 max_us={v} preloaded={PRELOADED[i]} synthetic_duration=1 gate_authority=0'
                       for i,v in enumerate(LIMITS))
        wire='SPARSEWIRE expected_epoch=7 synthetic=1 expected_n=16 expected_omitted=5\n'
        wire+='QUALSPARSE epoch=7 n=16 omitted=5 invalid=0 frozen=1 capture=1 threshold_us=40 dispatch_body=1 edge_time=0 normal_calls_counted=0\n'
        wire+='\n'.join(record('QS85',[7,0,i,0,i,1 if i==20 else 41,4 if i==20 else 3,
                                     int(i!=20),int(i==19),int(i==20)]) for i in range(5,21))
        return 'SPARSEBASE max_us=1 samples=16 subtracted=0\n'+rows+'\n'+wire+'\nSPARSECHECK END disabled=1\nFINALOFF\n'+final
    def test_limits_and_semantics(self):
        s=self.sample();self.assertTrue(verify(s)['outputs_off'])
        for bad in [s.replace('max_us=2','max_us=3'),s.replace('passed=16','passed=15',1),
                    s.replace('mode=1','mode=0'),s.replace('END disabled=1','END disabled=0'),
                    s.replace('FINALOFF','SPARSECHECK refused=1\nFINALOFF')]:
            with self.assertRaises(ValueError):verify(bad)
    def test_populated_coverage_required(self):
        s=self.sample()
        for bad in [s.replace('preloaded=32','preloaded=16'),
                    '\n'.join(line for line in s.splitlines() if not line.startswith('SPARSECHECK mode=7')),
                    s.replace('mode=6 passed=16 total=16 max_us=4','mode=6 passed=16 total=16 max_us=5')]:
            with self.assertRaises(ValueError):verify(bad)
    def test_off_required(self):
        with self.assertRaises((ValueError,RuntimeError)):
            verify(self.sample().split('FINALOFF')[0]+'FINALOFF\n')
    def test_wire_epoch_and_payload_required(self):
        for bad in [self.sample().replace('expected_epoch=7','expected_epoch=8'),
                    self.sample().replace('QS85 ','QS85 !',1),
                    self.sample().replace('expected_n=16','expected_n=15')]:
            with self.assertRaises(ValueError):verify(bad)

if __name__=='__main__':unittest.main()
