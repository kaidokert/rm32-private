import unittest
from pathlib import Path
from am32_response_check import decode_segments


class Segments(unittest.TestCase):
    sample=b'AM32SEGS 00000001 00000000\nAM32SEG 000000f7 000003ee 00000f97 00000a69 00000138 00000001 00000000\n'

    def test_decode(self):
        self.assertEqual(decode_segments(self.sample),[[247,1006,3991,2665,312,1,0]])
        self.assertEqual(decode_segments(b'AM32SEGS 00000000 00000000\n'),[])

    def test_retained_failure_is_present_at_first_boundary(self):
        raw=(Path(__file__).resolve().parents[1]/'captures/am32_531_segments_ramp10to30_01.txt').read_bytes()
        rows=decode_segments(raw[raw.rfind(b'AM32SEGS '):])
        self.assertEqual([r[0] for r in rows],[247,347,447,547])
        self.assertTrue(all(r[2]==0 and r[4]==0 and r[5]==0 and r[6]==102 for r in rows))

    def test_reject_incomplete_overflow_duplicate_and_overcap(self):
        for data in [b'', self.sample+self.sample,
                     self.sample.replace(b'00000001 00000000\n',b'00000001 00000001\n'),
                     self.sample.split(b'AM32SEG ')[0],
                     self.sample.replace(b'00000138',b'00000320')]:
            with self.subTest(data=data),self.assertRaises(ValueError):decode_segments(data)


if __name__=='__main__':unittest.main()
