import unittest
from drv_qualification_sparse import decode,decode_campaign
from test_drv_qualification_window import record

def sample(rows=None,epoch=7,omitted=0):
    if rows is None:rows=[[7,0,10,0,65530,48,3,0,1,0],[7,0,11,0,12,1,4,1,0,1]]
    h=f'QUALSPARSE epoch={epoch} n={len(rows)} omitted={omitted} invalid=0 frozen=1 capture=1 threshold_us=40 dispatch_body=1 edge_time=0 normal_calls_counted=0'
    return '\n'.join([h]+[record('QS85',r) for r in rows])

class SparseDecode(unittest.TestCase):
    def test_campaign_binding(self):
        binding='SPARSEBIND epoch=7 accepted=12 final_observation=1 ram=1\n'
        self.assertEqual(decode_campaign(binding+sample(),True)['final_accepted'],12)
        for text in [sample(),binding+binding+sample(),binding.replace('epoch=7','epoch=8')+sample(),
                     binding.replace('accepted=12','accepted=11')+sample(),
                     binding.replace('ram=1','ram=0')+sample(),
                     binding.replace('accepted=12','accepted=4294967296')+sample()]:
            with self.assertRaises(ValueError):decode_campaign(text,True)
        self.assertEqual(decode_campaign(binding+sample([]),True)['rows'],[])
    def test_valid_and_short_fault(self):
        r=decode(sample(),True,expected_epoch=7)
        self.assertTrue(r['rows'][-1]['stopped']);self.assertEqual(r['rows'][0]['duration_us'],48)
        self.assertFalse(r['entry_tick_unwrapped']);self.assertTrue(r['campaign_epoch_checked'])
    def test_empty_is_not_proof_of_no_timing_fault(self):
        r=decode(sample([]),True);self.assertEqual(r['rows'],[])
        self.assertFalse(r['normal_calls_counted']);self.assertFalse(r['campaign_epoch_checked'])
        self.assertIsNone(decode(''))
    def test_epoch_crc_and_header_rejection(self):
        for text in [sample().replace('epoch=7','epoch=0'),sample().replace('epoch=7','epoch=8'),
                     sample().replace('invalid=0','invalid=1'),sample().replace('threshold_us=40','threshold_us=41'),
                     sample().replace('QS85 ','QS85 !',1),sample()+'\nQS85',sample()+'\n'+sample(),
                     sample().replace('n=2','n=17')]:
            with self.assertRaises(ValueError):decode(text,True)
        with self.assertRaises(ValueError):decode(sample(),True,expected_epoch=8)
    def test_flag_trigger_identity_and_stop_rejection(self):
        base=[7,0,10,0,10,48,3,0,0,0]
        for index,value in [(6,0),(6,7),(7,2),(8,2),(9,2),(5,40)]:
            bad=base.copy();bad[index]=value
            with self.assertRaises(ValueError):decode(sample([bad]),True)
        for rows in [[base,base[:2]+[9]+base[3:]],
                     [base[:-1]+[1],base],
                     [[7,0,65535,65535,0,48,3,1,0,0]]]:
            with self.assertRaises(ValueError):decode(sample(rows),True)
    def test_omission_and_quiet_contract(self):
        rows=[[7,0,i,0,0,41,1,0,0,0] for i in range(16)]
        self.assertEqual(decode(sample(rows,omitted=50),True)['omitted_selected'],50)
        with self.assertRaises(ValueError):decode(sample(omitted=1),True)
        quiet=sample([]).replace('capture=1','capture=0')
        self.assertFalse(decode(quiet)['captured'])
        with self.assertRaises(ValueError):decode(quiet,True)
        with self.assertRaises(ValueError):decode('QS85 nonsense',True)

if __name__=='__main__':unittest.main()
