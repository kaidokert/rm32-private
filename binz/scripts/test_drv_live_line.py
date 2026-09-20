import unittest
from unittest.mock import Mock, patch
from drv_capture import send_live_line


class LiveLineTests(unittest.TestCase):
    def test_stop_is_byte_paced_and_terminated(self):
        port = Mock()
        with patch('drv_capture.time.sleep') as sleep:
            send_live_line(port, 'off')
        self.assertEqual([c.args[0] for c in port.write.call_args_list],
                         [b'o', b'f', b'f', b'\r'])
        self.assertEqual(port.flush.call_count, 4)
        self.assertEqual([c.args[0] for c in sleep.call_args_list], [.003]*4)

    def test_partial_line_can_be_terminated_without_command(self):
        port = Mock()
        with patch('drv_capture.time.sleep'):
            send_live_line(port, '')
        port.write.assert_called_once_with(b'\r')
