"""Host-only protocol tests; never opens a real serial port."""
import sys
import types
import base64
import struct
import zlib
from pathlib import Path
import unittest
from unittest.mock import patch

import drv_capture

class OffReadbackTests(unittest.TestCase):
    def test_sparse_hardware_capture_uses_original_tick_timestamps(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for tag,count,last in [('startup_direct200_sparse_01',246,4674),
                               ('startup_catch100_coast_01',48,912),
                               ('startup_downramp_coast_01',153,2907)]:
            text=(root/f'{tag}.txt').read_text()
            rows,hz,_,_,vcal,_=drv_capture.parse_dump(text)
            self.assertEqual(hz,52) # legacy integer approximation only
            self.assertIn('adc_hz=1000 retain_every_ticks=19',text)
            self.assertEqual([r['tick'] for r in rows],list(range(19,last+1,19)))
            self.assertEqual(len(rows),count)
            self.assertEqual(drv_capture.scale_rows(rows,vcal)[-1]['time_ms'],last)
            drv_capture.verify_off(text[text.rfind('OUT:'):].encode())

    GOOD=(b'OUT: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 TIM1:moe=0 ccrA=0 ccrB=0 ccrC=0\n'
          b'IN: ah=0 bh=0 ch=0 al=0 bl=0 cl=0 en=0 nflt=1\n')
    def test_complete_disabled_readback(self):
        drv_capture.verify_off(self.GOOD)
    def test_missing_or_active_fields_refuse(self):
        for reply in [b'',self.GOOD.split(b'IN:')[0]]+[
                self.GOOD.replace(a,b) for a,b in [(b'ah=0',b'ah=1'),(b'en=0',b'en=1'),
                    (b'TIM1:moe=0',b'TIM1:moe=1'),(b'ccrC=0',b'ccrC=1'),(b'nflt=1',b'nflt=0')]]:
            with self.assertRaises(RuntimeError): drv_capture.verify_off(reply)


class FakePort:
    def __init__(self):
        self.commands = []
        self.closed = False
        self.pending = ''

    def reset_input_buffer(self):
        pass

    def write(self, data):
        for char in data.decode():
            if char in '\r\n':
                if self.pending:
                    self.commands.append(self.pending)
                    self.pending = ''
            else:
                self.pending += char

    def flush(self):
        pass

    def read(self, size):
        return b"COAST END\n"

    def close(self):
        self.closed = True


class CaptureHandshake(unittest.TestCase):
    def exercise(self, acknowledgement):
        port = FakePort()
        serial = types.SimpleNamespace(Serial=lambda *a, **k: port)
        with patch.dict(sys.modules, serial=serial), patch.object(
            drv_capture, "read_available",
            side_effect=[b"", acknowledgement, b"", b"", b""],
        ):
            if acknowledgement:
                drv_capture.live_capture("FAKE", 115200, 50, 60, False, None)
            else:
                with self.assertRaisesRegex(RuntimeError, "opt-in handshake"):
                    drv_capture.live_capture("FAKE", 115200, 50, 60, False, None)
        self.assertTrue(port.closed)
        self.assertEqual(port.commands[-2:], ["off", "cap0"])
        return port.commands

    def test_fixture_arms_before_motion(self):
        commands = self.exercise(b"CAPTURE armed one-shot a85-v1\n")
        self.assertEqual(commands, ["off", "cap1", "du60", "run50", "off", "cap0"])

    def test_old_firmware_rejected_before_motion(self):
        commands = self.exercise(b"")
        self.assertNotIn("run50", commands)


class SnapshotCodec(unittest.TestCase):
    @staticmethod
    def frame(kind, values):
        raw = struct.pack(f"<{len(values)}H", *values)
        raw += struct.pack("<I", zlib.crc32(raw))
        return kind + " " + base64.a85encode(raw).decode()

    def compact_reference(self):
        path = Path(__file__).resolve().parent.parent / "captures/shell_pwm_validation_03.txt"
        text = path.read_text()
        rows, _, coast, _, _, _ = drv_capture.parse_dump(text)
        lines = ["WIRE a85-v1", drv_capture.HEADER_RE.search(text).group()]
        lines += [self.frame("D85", [r[k] for k in drv_capture.FIELDS]) for r in rows]
        lines += ["CAP END", drv_capture.COAST_HEADER_RE.search(text).group()]
        lines += [self.frame("C85", [r[k] for k in drv_capture.COAST_FIELDS] +
                            [r['elapsed_us'] & 65535, r['elapsed_us'] >> 16]) for r in coast]
        lines += ["COAST END"]
        return text, "\n".join(lines)

    def test_real_capture_roundtrip(self):
        old, compact = self.compact_reference()
        self.assertEqual(drv_capture.parse_dump(old), drv_capture.parse_dump(compact))
        self.assertLess(len(compact), len(old))

    def test_corruption_and_truncation(self):
        _, compact = self.compact_reference()
        lines = compact.splitlines()
        record = lines[2]
        damaged = record[:-1] + ('!' if record[-1] != '!' else '#')
        for bad in [compact.replace(record, damaged, 1),
                    compact.replace(record + '\n', '', 1),
                    compact.replace('COAST END', ''),
                    compact.replace('WIRE a85-v1', 'WIRE a85-v9')]:
            with self.assertRaises(RuntimeError):
                drv_capture.parse_dump(bad)

    def test_rust_vector(self):
        # Produced by the firmware's host-compiled encoder.
        raw = base64.a85decode(b'!!!$"s8P+/=LS')
        self.assertEqual(raw[:6], struct.pack('<3H', 0, 1, 65535))
        self.assertEqual(zlib.crc32(raw[:6]), struct.unpack('<I', raw[6:])[0])


if __name__ == "__main__":
    unittest.main()
