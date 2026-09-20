import base64
import struct
import unittest
import zlib
from pathlib import Path
from drv_irq_trace import decode,evidence_summary,gate_accept_timing,rejection_cadence

def record(gate=1200,reads=1,last=1,accepted=0):
    raw=struct.pack('<14H',1,199,2,562,gate,1666,reads,last,last,0,0,accepted,45,0)
    return 'I85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()

class IrqTraceTests(unittest.TestCase):
    def test_retained_pass_and_failure_both_have_100us_rejection_gaps(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['irqtail_hold54_01.txt','irqtail_hold55_01.txt',
                     'cachedcomp_hold54_01.txt','cachedcomp_hold55_01.txt']:
            with self.subTest(name=name):
                report=rejection_cadence((root/name).read_text())
                self.assertIn(100,[r['entry_gap_us'] for r in report['pairs']])
                self.assertFalse(report['preemption_proven'])

    def test_rejection_cadence_does_not_bridge_missing_visits(self):
        def row(seq,us,step=2,accepts=0):
            raw=struct.pack('<15H',seq,us&65535,step,562,1200,1100,1,0,0,
                            0,0,accepts,12,1,us>>16)
            return 'I85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()
        text=('IRQWINDOW mode=tail time_bits=32 capacity=24 appended=us_hi\n'
              'IRQTRACE n=6 drop=0 fields=tail\n'+
              '\n'.join([row(65535,70000),row(0,70042),row(2,70084),
                         row(3,70126,3),row(4,70168,3,1),row(5,70210,3)]))
        report=rejection_cadence(text)
        self.assertEqual([p['entry_gap_us'] for p in report['pairs']],[42])
        self.assertFalse(report['preemption_proven'])
        self.assertFalse(report['physical_edge_intervals'])
        with self.assertRaises(ValueError):rejection_cadence(record())

    def test_inline_read_provenance(self):
        marker='COMPREAD inline_adapter=1 sample_count_unchanged=1 signal_cached=0\n'
        decode(marker+record())
        for bad in [marker+marker,marker.replace('signal_cached=0','signal_cached=1'),
                    marker.replace('sample_count_unchanged=1','sample_count_unchanged=0')]:
            with self.assertRaises(ValueError):decode(bad+record())

    def test_gate_accept_brackets_use_same_capture_and_clock(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'irqtail_hold55_01.txt').read_text()
        r=gate_accept_timing(text)
        self.assertEqual([e['gate_to_accept_us'] for e in r['rows']],[37,37,46.5,37,67])
        self.assertFalse(r['pure_filter_duration'])
        self.assertFalse(r['refused_event_included'])
        with self.assertRaises(ValueError):
            gate_accept_timing((root/'cycle_core_hold54_01.txt').read_text())

    def test_cached_mode_provenance(self):
        marker='COMPMODE cached_per_call=1 signal_cached=0 safety_cached=0\n'
        decode(marker+record())
        for bad in [marker+marker,marker.replace('signal_cached=0','signal_cached=1')]:
            with self.assertRaises(ValueError):decode(bad+record())

    def test_actual_cached_adapter_brackets_and_refusal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'cachedcomp_hold54_01.txt').read_text()
        self.assertIn('COMPMODE cached_per_call=1 signal_cached=0 safety_cached=0',text)
        self.assertEqual([r['gate_to_accept_us'] for r in gate_accept_timing(text)['rows']],
                         [29.5,38.5,47.5,39,29.5])
        failed=(root/'cachedcomp_hold55_01.txt').read_text()
        last=decode(failed)[-1]
        self.assertEqual((last['step'],last['reads'],last['accepts']),(5,12,0))
        self.assertEqual(last['outcome'],'unresolved')
    def test_tail_full_width_time_and_sequence_wrap(self):
        def row(seq,us):
            raw=struct.pack('<15H',seq,us&65535,2,562,1200,1100,12,1,1,0,1,1,80,1,us>>16)
            return 'I85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()
        header='IRQWINDOW mode=tail time_bits=32 capacity=24 appended=us_hi\nIRQTRACE n=2 drop=0 fields=tail\n'
        text=header+row(65535,4404500)+'\n'+row(0,4404600)
        self.assertEqual([r['us'] for r in decode(text)],[4404500,4404600])
        self.assertEqual(evidence_summary(text)['window'],'tail')
        for bad in [text.replace('drop=0','drop=1'),text.replace('time_bits=32','time_bits=16'),
                    header+row(0,4404600)+'\n'+row(1,4404500),
                    text+'\nIRQWINDOW mode=tail time_bits=32 capacity=24 appended=us_hi',
                    '\n'.join(text.splitlines()[1:])]:
            with self.subTest(bad=bad),self.assertRaises(ValueError):decode(bad)

    def test_full_summary_rejects_tail_metadata_on_legacy_rows(self):
        from drv_sustained_report import summarize
        text=(Path(__file__).resolve().parents[1]/'captures/cycle_core_hold54_01.txt').read_text()
        summarize(text)
        with self.assertRaises(ValueError):
            summarize(text+'\nIRQWINDOW mode=tail time_bits=32 capacity=24 appended=us_hi\n')

    def test_actual_tail_after_multiwrap_run_and_guard_refusal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        healthy=decode((root/'irqtail_hold54_01.txt').read_text())
        self.assertEqual(len(healthy),24)
        self.assertGreater(healthy[0]['us'],9_000_000)
        self.assertEqual(healthy[-1]['seq'],80962%65536)
        failed=decode((root/'irqtail_hold55_01.txt').read_text())
        self.assertEqual([r['seq'] for r in failed],list(range(3525,3549)))
        last=failed[-1]
        self.assertEqual((last['us'],last['step'],last['reads']),(439004,2,12))
        # Reference persistence passed; safety callback prevented recording.
        # accepts=0 alone must not be labeled persistence rejection.
        self.assertEqual(last['first'],last['rising'])
        self.assertEqual(last['last'],last['rising'])
        self.assertEqual(last['outcome'],'unresolved')

    def test_equal_endpoints_are_not_a_fresh_crossing_or_qzc_certificate(self):
        text='IRQTRACE n=1 drop=100 fields=legacy\n'+record(reads=12,accepted=1)
        result=evidence_summary(text)
        self.assertEqual(result['accepted_without_observed_endpoint_change'],1)
        self.assertEqual(result['endpoint_change_observed'],0)
        self.assertEqual(result['omitted_handlers'],100)
        self.assertFalse(result['qzc_measured'])
        with self.assertRaises(ValueError): evidence_summary(record())

    def test_real_powered_prefix_and_disabled_trace(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'handoff_trace1_01.txt').read_text()
        rows=decode(text)
        self.assertEqual(len(rows),32)
        self.assertEqual([r['seq'] for r in rows],list(range(1,33)))
        self.assertEqual([r['reads'] for r in rows if r['accepts']],[12,12])
        self.assertTrue(any(r['outcome']=='persistence_reject' and r['reads']>1 for r in rows))
        self.assertEqual(decode((root/'handoff_trace0_01.txt').read_text()),[])
        with self.assertRaises(ValueError): decode(text.replace('IRQTRACE n=32','IRQTRACE n=31'))
    def test_paths(self):
        for text,want in [(record(),'persistence_reject'),(record(gate=833),'blank_gate'),
                          (record(gate=65535,reads=0),'no_gate_read'),(record(accepted=1),'accepted')]:
            self.assertEqual(decode(text)[0]['outcome'],want)
    def test_bad_crc(self):
        text=record()
        with self.assertRaises(ValueError):
            decode(text[:-1]+('!' if text[-1]!='!' else '"'))
