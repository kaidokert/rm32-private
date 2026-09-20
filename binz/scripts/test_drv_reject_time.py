import unittest
from drv_reject_time import decode
from test_drv_qualification_direct import sample,row
from test_drv_qualification_window import record


def fixture():
    direct=sample([row(0,1|(1<<15)),row(1,2),row(2,0,1,(0,0,0,0,0))]).replace('qd85-v1','qd85-v2')
    head='REJECTTIME epoch=7 n=1 total=1 omitted=0 final_accepted=2 invalid=0 callback_coordinates=1 wire=rt85-v1\n'
    frame=[7,0,0,0,0,0,1,11,600,630,1000,19,632,0]
    return direct+'\n'+head,frame


class RejectTimeTests(unittest.TestCase):
    def test_coordinates_are_not_edge_or_preemption_proof(self):
        text,frame=fixture();r=decode(text+record('RT85',frame),True,campaign=False)
        self.assertEqual(r['rows'][0]['bracket_delta_mod65536'],2)
        self.assertFalse(r['physical_edge_time']);self.assertFalse(r['preemption_proven'])
        with self.assertRaises(ValueError):decode(text+record('RT85',frame),True)

    def test_corruption_and_identity_and_bad_fields_refuse(self):
        text,frame=fixture()
        for field,value in [(0,8),(2,1),(4,3),(6,0),(6,5),(7,0),(7,12),(8,0),(13,1)]:
            bad=frame.copy();bad[field]=value
            with self.assertRaises(ValueError):decode(text+record('RT85',bad),True,campaign=False)
        with self.assertRaises(ValueError):decode(text+record('RT85',frame).rstrip()+'!',True,campaign=False)

    def test_optional_missing_and_unknown_version(self):
        self.assertIsNone(decode(''))
        text,frame=fixture()
        with self.assertRaises(ValueError):decode(text.replace('rt85-v1','rt85-v2')+record('RT85',frame),True,campaign=False)
