import base64
import struct
import unittest
import zlib
from drv_comp_decisions import decode


class DecisionTests(unittest.TestCase):
    def test_retained_recovery_fault_keeps_stopped_decision(self):
        from pathlib import Path
        from drv_cycle_fault import context
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/decision_start61_reentry68_30s_01.txt').read_text()
        tail=decode(text,True)['rows'];fault=context(text)
        self.assertEqual(tail[-1]['path'],4)
        self.assertEqual(fault['guard']['delta_us'],3015)
        self.assertEqual(fault['controller']['this_zc_ticks']-tail[-1]['count'],13)
        last_accepted=[r for r in tail if r['path']==3][-1]
        self.assertEqual(fault['controller']['last_zc_ticks']-last_accepted['count'],13)
        with self.assertRaises(ValueError):verify(text,30000,dropout=True,reentry=True)

    def test_retained_powered_tail_and_aggregate_agree(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/decision_start61_hold68_01.txt').read_text()
        result=decode(text,True)
        self.assertEqual(len(result['rows']),32)
        self.assertEqual(result['omitted'],165172)
        self.assertEqual(result['rows'][-1]['seq'],165204)
        self.assertEqual(verify(text,10000)['acquisition']['duty_tenths'],61)
        with self.assertRaisesRegex(ValueError,'epoch accounting'):
            decode(text.replace('closed=29792','closed=29793'),True)

    def test_retained_startup_current_refusal_did_not_exercise_recorder(self):
        from pathlib import Path
        from drv_driven_run import records
        from drv_driven_handoff import verify
        from drv_capture import verify_off
        text=(Path(__file__).resolve().parents[1]/'captures/decision_hold68_01.txt').read_text()
        rows=records(text,'DA85',7)
        self.assertEqual(len(rows),70)
        self.assertEqual(max(abs(v-2048) for r in rows for v in r[2:5]),1256)
        self.assertEqual(rows[-1][4],3304)
        self.assertEqual(decode(text)['rows'],[])
        with self.assertRaisesRegex(ValueError,'unused'):decode(text,True)
        with self.assertRaises(ValueError):verify(text,10000)
        verify_off(text[text.rfind('OUT:'):].encode())

    def test_framing_and_classification(self):
        def capture(seq=1,meta=1|(2<<8)|(1<<16),count=501):
            raw=struct.pack('<4I',123,seq,count|(1000<<16),meta)
            record=base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()
            return ('COMPDECISION n=1 total=1 omitted=0 invalid=0 frozen=1 active=0 capture=1 entry_not_edge=1 wire=d85-v1\nCD85 '+record)
        good=capture()
        self.assertEqual(decode(good,True)['rows'][0]['path'],2)
        self.assertFalse(decode(good)['physical_edge_timestamps'])
        quiet=good.splitlines()[0].replace('capture=1','capture=0')
        self.assertFalse(decode(quiet)['captured'])
        with self.assertRaises(ValueError):decode(quiet,True)
        with self.assertRaises(ValueError):decode(good.splitlines()[1])
        for bad in [capture(seq=2),capture(count=500),capture(meta=0),
                    good.replace('invalid=0','invalid=1'),good+'\n'+good,
                    good.replace('CD85','D85'),good.replace('omitted=0','omitted=1')]:
            with self.assertRaises(ValueError):decode(bad,True)


if __name__=='__main__':unittest.main()
