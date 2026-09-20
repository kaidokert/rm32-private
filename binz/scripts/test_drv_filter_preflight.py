import unittest
from unittest.mock import patch
import subprocess
import drv_filter_preflight

class PreflightTests(unittest.TestCase):
    def test_failure_stops_sequence(self):
        with patch('sys.argv',['preflight','--prefix','unused']),patch.object(
            drv_filter_preflight.subprocess,'run',side_effect=[None,None,None,
                subprocess.CalledProcessError(1,['cpu'])]) as run:
            with self.assertRaises(subprocess.CalledProcessError):drv_filter_preflight.main()
            self.assertEqual(run.call_count,4)
            self.assertTrue(all(call.kwargs['check'] for call in run.call_args_list))
    def test_success_is_disabled_only(self):
        with patch('sys.argv',['preflight','--prefix','unused']),patch.object(
            drv_filter_preflight.subprocess,'run') as run,patch('builtins.print'):
            drv_filter_preflight.main()
            self.assertEqual(run.call_count,5)
            for call in run.call_args_list:
                self.assertNotIn('drv_driven_handoff.py',' '.join(call.args[0]))
