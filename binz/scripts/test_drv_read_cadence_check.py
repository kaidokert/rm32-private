import unittest
from pathlib import Path
from drv_read_cadence_check import verify


class ReadCadence(unittest.TestCase):
    def test_retained_inline_and_refusals(self):
        text=(Path(__file__).resolve().parents[1]/'captures/compinline_537_read01.txt').read_text()
        self.assertEqual(verify(text,False)['max_us'],4)
        for changed,call in [(text,True),(text.replace('max_us=4','max_us=11'),False),
                             (text.replace('reads=12','reads=11'),False),
                             (text.replace('FINALOFF','INVALID'),False)]:
            with self.subTest(call=call),self.assertRaises(ValueError):verify(changed,call)
