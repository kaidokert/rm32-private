import unittest
from drv_qualification_event import decode
from test_drv_qualification_window import record


def row(identity,step,counts=None,partial=False):
    counts=[0,2,3,1,0] if counts is None else counts
    present=bool(counts[2]+counts[3]); stopped=bool(counts[4])
    return [7,0,identity&65535,identity>>16,*counts,
            600 if present else 0,900 if present else 0,
            step|int(present)<<8|int(stopped)<<9,int(partial)]


def sample(rows=None,omitted=0,final=None):
    if rows is None: rows=[row(0,1),row(1,2),row(2,0,[0]*5,True)]
    n=len(rows)-1; total=n+omitted
    if final is None: final=total
    return (f'QUALEVENT epoch=7 n={n} total={total} omitted={omitted} invalid=0 frozen=1 final_accepted={final} dispatched_only=1 wire=qe85-v1\n'
            +'\n'.join(record('QE85',r) for r in rows))


class EventDecoderTests(unittest.TestCase):
    def test_valid_empty_and_binding(self):
        r=decode(sample(),True,7,2)
        self.assertEqual(len(r['rows']),2);self.assertTrue(r['accepted_checked'])
        self.assertEqual(decode(sample([row(0,0,[0]*5,True)]),True)['rows'],[])
        self.assertIsNone(decode(''))
        for kwargs in [dict(expected_epoch=8),dict(expected_accepted=3)]:
            with self.assertRaises(ValueError): decode(sample(),True,**kwargs)

    def test_stopped_can_hide_committed_accept_but_not_prove_persistence(self):
        stop=row(1,2,[0,1,2,0,1]); partial=stop.copy();partial[-1]=1
        for final in [1,2]:
            r=decode(sample([row(0,1),stop,partial],final=final),True)
            self.assertTrue(r['stopped_callback'])
            self.assertFalse(r['stopped_reference_persistence_known'])
        with self.assertRaises(ValueError):decode(sample([row(0,1),stop,partial],final=3),True)

    def test_wrapped_suffix(self):
        rows=[row(i,i%6+1) for i in range(24,40)]+[row(40,5,[1,2,0,0,0],True)]
        self.assertEqual(decode(sample(rows,omitted=24),True)['omitted'],24)
        with self.assertRaises(ValueError):decode(sample(rows,omitted=23),True)

    def test_crc_header_and_semantics_reject(self):
        for text in [sample()+'\nQE85 bad',sample()+'\n'+sample(),
                     sample().replace('invalid=0','invalid=1'),
                     sample().replace('QE85 ','QE85 !',1),
                     sample().replace('epoch=7','epoch=8')]:
            with self.assertRaises(ValueError):decode(text,True)
        for index,value in [(0,8),(2,4),(7,2),(8,1),(11,0),(11,1025),(12,1)]:
            bad=row(0,1);bad[index]=value
            with self.assertRaises(ValueError):decode(sample([bad,row(1,0,[0]*5,True)]),True)
        with self.assertRaises(ValueError):decode(sample([row(0,1),row(1,3),row(2,0,[0]*5,True)]),True)
