import unittest
from pathlib import Path
from drv_accepted_events import decode_windows,decode_first_segment

class ArchiveTests(unittest.TestCase):
    def test_hardware_archive_matches_original_stopped_stream(self):
        raw=(Path(__file__).resolve().parents[1]/'captures/stack_archive_01.txt').read_text()
        saved=decode_first_segment(raw);primary=decode_windows(raw)
        for key in ('prefix','tail','skipped','total'): self.assertEqual(saved[key],primary[key])
        self.assertEqual(saved['header']['fault'],8)
        self.assertEqual(saved['header']['commits'],2463)

    def fixture(self):
        # Format regression built from real recorded CRC frames; NOT evidence
        # that the new firmware's archive was exercised on hardware.
        raw=(Path(__file__).resolve().parents[1]/'captures/sustain40_60s_01.txt').read_text()
        w=decode_windows(raw)
        header=f"FIRSTSEG fault=8 events={w['total']} prefix={len(w['prefix'])} tail={len(w['tail'])} skipped={w['skipped']} end_us=60000044"
        rows=[('P185 '+line[4:]) if line.startswith('A85 ') else ('T185 '+line[4:])
              for line in raw.splitlines() if line.startswith(('A85 ','T85 '))]
        return header+'\n'+'\n'.join(rows),w
    def test_frames_preserve_windows_and_full_width_counts(self):
        text,w=self.fixture();saved=decode_first_segment(text)
        for key in ('prefix','tail','skipped','total'): self.assertEqual(saved[key],w[key])
    def test_truncation_duplicate_header_and_crc_corruption_reject(self):
        text,_=self.fixture()
        damaged=text.replace('P185 ','P185 !',1)
        for changed in (text.rsplit('\n',1)[0],text+'\n'+text.splitlines()[0],damaged,
                        text.replace('end_us=60000044','end_us=1')):
            with self.assertRaises(ValueError): decode_first_segment(changed)

if __name__=='__main__': unittest.main()
