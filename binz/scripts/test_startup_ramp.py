import unittest
from unittest.mock import patch
import drv_capture


class StartupRampTests(unittest.TestCase):
    def test_step_requires_exact_ack_and_rejections_are_not_success(self):
        self.assertTrue(drv_capture.step_ack(b'echo\r\nF60\r\n',60))
        self.assertFalse(drv_capture.step_ack(b'F160\r\n',60))
        self.assertFalse(drv_capture.step_ack(b'F60',60))
        for reply in [b'!step\r\n',b'!hz\r\n',b'!busy\r\n']:
            with self.assertRaises(RuntimeError):
                drv_capture.step_ack(reply,60)

    def test_invalid_ramps_reject_before_opening_uart(self):
        for start, span in [(0, 2), (3.3, 1), (.2, 0), (1, 4), (float('nan'), 2)]:
            with self.subTest(start=start, span=span), patch('serial.Serial') as uart:
                with self.assertRaises(ValueError):
                    drv_capture.live_capture('unused', 115200, 50, 62, False, None,
                                             step_start_s=start, step_span_s=span)
                uart.assert_not_called()

    def test_default_and_early_ramps_pass_validation(self):
        for start, span in [(3.2, 1.2), (.2, 3), (.2, 2)]:
            with self.subTest(start=start, span=span), patch('serial.Serial', side_effect=RuntimeError('test UART boundary')):
                with self.assertRaisesRegex(RuntimeError, 'test UART boundary'):
                    drv_capture.live_capture('unused', 115200, 50, 62, False, None,
                                             step_start_s=start, step_span_s=span)


if __name__ == '__main__':
    unittest.main()
