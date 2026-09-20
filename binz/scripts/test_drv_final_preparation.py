import unittest
from drv_final_preparation import verify

class FinalPreparation(unittest.TestCase):
    marker='FINALPREP used=1 interval=11 output_authority=0 final_edge_checks=1\n'
    def test_explicit_selection_and_old_build(self):
        self.assertIsNone(verify('old capture\n'))
        with self.assertRaisesRegex(ValueError,'missing'):verify('',True)
        with self.assertRaisesRegex(ValueError,'explicit'):verify(self.marker)
        self.assertTrue(verify(self.marker,True,used=True))
    def test_malformed_and_duplicate(self):
        for text in [self.marker*2,'FINALPREP\n',self.marker.replace('used=1','used=2'),
                     self.marker.replace('interval=11','interval=12'),
                     self.marker.replace('output_authority=0','output_authority=1'),
                     self.marker.replace('checks=1','checks=0'),self.marker.rstrip()+' extra\n']:
            with self.assertRaises(ValueError):verify(text,True)
    def test_consumption_not_success(self):
        with self.assertRaisesRegex(ValueError,'disagrees'):verify(self.marker,True,used=False)
        unused=self.marker.replace('used=1','used=0')
        self.assertFalse(verify(unused,True,used=False))
        with self.assertRaisesRegex(ValueError,'disagrees'):verify(unused,True,used=True)
        # Marker parser only returns provenance, even when an arm refusal exists.
        self.assertTrue(verify(self.marker+'CORESEED armed=0 refusal_stop_code=8\n',True))
