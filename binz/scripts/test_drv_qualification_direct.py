import unittest
from drv_qualification_direct import decode,decode_campaign
from test_drv_qualification_window import record


def row(identity,step,partial=0,counts=(4,1,2,600,900)):
    return [7,0,identity&65535,identity>>16,step,*counts,partial]


def sample(rows=None,total=2):
    if rows is None:rows=[row(0,1),row(1,2),row(2,0,1,(0,0,0,0,0))]
    n=min(total,16)
    return (f'QUALDIRECT epoch=7 n={n} total={total} omitted={total-n} invalid=0 active=0 final_accepted={total} dispatched_only=1 partial_until_unwind=1 wire=qd85-v1\n'
            +'\n'.join(record('QD85',r) for r in rows))


class DirectDecoderTests(unittest.TestCase):
    def test_v2_rejection_positions_and_v1_never_reinterpreted(self):
        rows=[row(0,1|(1<<4)),row(1,2|(1<<15)),row(2,0,1,(1,0,1,700,700))]
        rows[-1][4]=1<<15
        text=sample(rows).replace('qd85-v1','qd85-v2')
        r=decode(text,True)
        self.assertEqual(r['rows'][0]['rejected_read_indices'],[0])
        self.assertEqual(r['rows'][1]['rejected_read_indices'],[11])
        self.assertEqual(r['partial']['rejected_read_indices'],[11])
        with self.assertRaises(ValueError):decode(sample(rows),True)
        rows[0][4]=1|(3<<4) # two different rejects plus acceptance need >=3 open visits
        with self.assertRaises(ValueError):decode(sample(rows).replace('qd85-v1','qd85-v2'),True)
        with self.assertRaises(ValueError):decode(text.replace('qd85-v2','qd85-v3'),True)

    def test_campaign_binding_is_required_even_for_optional_present_data(self):
        binding='DIRECTBIND epoch=7 accepted=2 final_observation=1 ram=1\n'
        quality='ACCEPTQUALITY events=2 first_us=1 last_us=2 order_bad=0 end_us=3 lock_proven=0\n'
        text=binding+quality+sample()
        result=decode_campaign(text,True)
        self.assertTrue(result['epoch_checked'] and result['accepted_checked'])
        self.assertTrue(result['final_observation_only'])
        self.assertFalse(result['campaign_lock_proven'])
        self.assertIsNone(decode_campaign(''))
        for bad in [sample(),quality+sample(),binding+sample(),binding+text,
                    quality+text,text.replace('DIRECTBIND epoch=7','DIRECTBIND epoch=8'),
                    text.replace('accepted=2 final_observation','accepted=3 final_observation'),
                    text.replace('events=2','events=3'),text.replace('ram=1','ram=0'),
                    text.replace('final_observation=1','final_observation=0')]:
            with self.assertRaises(ValueError):decode_campaign(bad)
        with self.assertRaises(ValueError):decode_campaign('',True)

    def test_valid_and_empty_partial(self):
        r=decode(sample(),True,7,2)
        self.assertEqual(r['rows'][0]['no_first_read'],1)
        self.assertTrue(r['epoch_checked']);self.assertFalse(r['persistence_reject_count_known'])
        empty=sample([row(0,0,1,(0,0,0,0,0))],0)
        self.assertEqual(decode(empty,True)['rows'],[])
        self.assertIsNone(decode(''))

    def test_wrap_and_uncommitted_open_partial(self):
        rows=[row(i,i%6+1) for i in range(24,40)]+[row(40,0,1)]
        r=decode(sample(rows,40),True)
        self.assertEqual(r['omitted'],24)
        self.assertEqual(r['partial']['open'],2)
        self.assertEqual(r['final_accepted'],40)

    def test_header_crc_binding_and_legacy_rejected(self):
        for text in [sample().replace('active=0','active=1'),sample().replace('invalid=0','invalid=1'),
                     sample().replace('epoch=7','epoch=0'),sample()+'\n'+sample(),
                     sample().replace('QD85 ','QD85 !',1),sample()+'\nQD85',
                     sample().replace('final_accepted=2','final_accepted=3'),'QE85 wrong']:
            with self.assertRaises(ValueError):decode(text,True)
        with self.assertRaises(ValueError):decode(sample(),True,8,2)
        with self.assertRaises(ValueError):decode(sample(),True,7,3)

    def test_identity_flags_and_counts(self):
        for index,value in [(0,8),(2,5),(4,0),(4,7),(5,2),(7,0),(8,0),(10,1)]:
            bad=row(0,1);bad[index]=value
            with self.assertRaises(ValueError):decode(sample([bad,row(1,2),row(2,0,1,(0,0,0,0,0))]),True)
        with self.assertRaises(ValueError):decode(sample([row(0,1),row(1,3),row(2,0,1)]),True)
        with self.assertRaises(ValueError):decode(sample([row(0,1),row(1,2),row(2,3,1)]),True)
