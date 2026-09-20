import contextlib
import io
import unittest
from unittest.mock import patch
from live_armed_baseline import main,verify_running_profile

class LiveProfile(unittest.TestCase):
    def test_seed_conflict_rejected_before_file_or_uart(self):
        with patch('sys.argv',['live','--out','unused.txt','--seed400','--seed500']), \
             patch('pathlib.Path.open') as opened, \
             patch('drv_capture.live_capture') as capture, \
             contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as e:main()
            self.assertEqual(e.exception.code,2)
            opened.assert_not_called();capture.assert_not_called()

    def test_exact_profiles_and_explicit_selection(self):
        for floor,c400,c450 in [(2778,False,False),(2500,True,False),(2223,False,True)]:
            text=f'RUNLIMIT cycle_min_us={floor} event_min_us=238 cycle_max_us=6000 event_max_us=1000 experimental=1\n'
            verify_running_profile(text,c400,c450)
            for bad in [text+text,text.replace('238','237'),text.replace('6000','6001'),'']:
                with self.assertRaises(ValueError):verify_running_profile(bad,c400,c450)
        with self.assertRaises(ValueError):verify_running_profile(text)
        with self.assertRaises(ValueError):verify_running_profile(text,True,True)

    def test_conflict_rejected_before_file_or_uart(self):
        with patch('sys.argv',['live','--out','unused.txt','--cycle400','--cycle450']), \
             patch('pathlib.Path.open') as opened, \
             patch('drv_capture.live_capture') as capture, \
             contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as e:main()
            self.assertEqual(e.exception.code,2)
            opened.assert_not_called();capture.assert_not_called()
