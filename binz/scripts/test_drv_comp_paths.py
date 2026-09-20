import unittest
from drv_comp_paths import decode

ROW = ('COMPPATH scope=dispatched first_actual_count=1 no_gate=2 closed=30 '
       'open_no_accept=7 accepted=10 stopped_or_unknown=1 active=0')


class CompPathsTests(unittest.TestCase):
    def test_optional_and_exact_counts(self):
        self.assertIsNone(decode('old capture'))
        with self.assertRaises(ValueError):
            decode('old capture', required=True)
        result=decode(ROW, required=True)
        self.assertEqual(result['dispatched'],50)
        self.assertEqual(result['closed'],30)
        self.assertEqual(result['stopped_or_unknown'],1)

    def test_malformed_duplicate_active_and_saturated_refused(self):
        for text in [ROW+'\n'+ROW, ROW.replace('active=0','active=1'),
                     ROW.replace('closed=30','closed=-1'),
                     ROW.replace('closed=30','closed=4294967295'),
                     ROW.replace('scope=dispatched','scope=all'), 'COMPPATH']:
            with self.subTest(text=text), self.assertRaises(ValueError):
                decode(text)


if __name__=='__main__':unittest.main()
