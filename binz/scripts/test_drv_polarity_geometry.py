import unittest
from pathlib import Path
from drv_polarity_geometry import audit


class PolarityGeometryTests(unittest.TestCase):
    def test_all_six_raw_edges_are_opposite_reference_parity(self):
        source=(Path(__file__).resolve().parents[1]/'examples/support/sine_table.rs').read_text()
        rows=audit(source)
        self.assertEqual([r['raw_post_level'] for r in rows],[0,1,0,1,0,1])
        self.assertEqual([r['floating'] for r in rows],list('CAB CAB'.replace(' ','')))
        self.assertTrue(all(not r['matches'] for r in rows))
