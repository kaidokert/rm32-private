import unittest
from drv_driven_handoff import verify_com_keep_running


class ComKeepRunning(unittest.TestCase):
    marker = 'COMARM running_counter=1 stale_pending_cleared=1 stopped_start_retained=1 experimental=1\n'

    def test_explicit_mode(self):
        verify_com_keep_running('')
        verify_com_keep_running(self.marker, True)

    def test_refuses_missing_unexpected_duplicate_or_malformed(self):
        for text, required in [(self.marker, False), ('', True),
                               (self.marker * 2, True), ('COMARM\n', True),
                               (self.marker.replace('cleared=1', 'cleared=0'), True)]:
            with self.subTest(text=text, required=required), self.assertRaises(ValueError):
                verify_com_keep_running(text, required)
