import base64
import struct
import unittest
from pathlib import Path
import zlib
from drv_prestart_baseline import decode


def fixture(n=128):
    text='PREBASE status=2 n_target=128 elapsed_us=20000 entry_same_epoch=1 stationary_verified=0 offsets_applied=0 guard_settings_unchanged=1\n'
    for ch in [4,1,0,6,13]:
        total=n*2048;squares=n*2048**2
        words=[ch,n,total&65535,total>>16,*[(squares>>(16*i))&65535 for i in range(4)],2048,2048]
        raw=struct.pack('<10H',*words)
        text+='BZ85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'
    return text


class PrestartTests(unittest.TestCase):
    def test_real_recovery_revokes_initial_baseline(self):
        from drv_driven_handoff import verify
        path=Path(__file__).resolve().parents[1]/'captures/prebase_epoch_reentry53_01.txt'
        text=path.read_text()
        result=decode(text)
        self.assertEqual(result['header']['entry_same_epoch'],1)
        self.assertEqual(result['epoch'],dict(recovery_checked=1,recovery_matches=0,initial_records_only=1))
        self.assertFalse(result['calibrated_current'])
        self.assertTrue(verify(text,10000,dropout=True,reentry=True)['powered_recovery_verified'])
        with self.assertRaises(ValueError):
            verify(text.replace('recovery_matches=0','recovery_matches=1'),10000,dropout=True,reentry=True)

    def test_recovery_requires_revocation_when_marker_present(self):
        text=fixture()+'REENTRY result=7 first_injection_us=2000000\nBASEEPOCH recovery_checked=1 recovery_matches=0 initial_records_only=1\n'
        self.assertEqual(decode(text)['epoch']['recovery_matches'],0)
        for bad in [text.replace('recovery_matches=0','recovery_matches=1'),text.replace('recovery_checked=1','recovery_checked=0'),text+text.splitlines()[-1]+'\n']:
            with self.assertRaises(ValueError):decode(bad)
    def test_real_same_epoch_and_refused_crlf_attempt(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        good=decode((root/'prebase_handoff53_02.txt').read_text())
        self.assertEqual(good['header']['entry_same_epoch'],1)
        self.assertEqual(good['header']['elapsed_us'],13690)
        self.assertTrue(all(r['n']==128 for r in good['channels']))
        bad=decode((root/'prebase_handoff53_01.txt').read_text())
        self.assertEqual(bad['header']['status'],1)
        self.assertTrue(all(r['n']==0 for r in bad['channels']))
    def test_complete_is_not_calibrated(self):
        self.assertFalse(decode(fixture())['calibrated_current'])
        self.assertIsNone(decode('legacy'))
    def test_refuse_bad_evidence(self):
        for text in [fixture(127),fixture()+fixture(),fixture().replace('elapsed_us=20000','elapsed_us=50000'),fixture().replace('BZ85 ','MISSING ',1),fixture().replace('offsets_applied=0','offsets_applied=1')]:
            with self.assertRaises(ValueError): decode(text)
