import unittest
from drv_driven_handoff import verify_peer_priority


class DmaPeerTests(unittest.TestCase):
    def test_dma_peer_does_not_erase_recovered_cycle_fault(self):
        from pathlib import Path
        from drv_cycle_fault import context
        from drv_driven_handoff import verify
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'dmapeer_reentry69_30s_01.txt').read_text()
        verify_peer_priority(text,True,dma_peer=True)
        result=context(text)
        self.assertEqual(result['guard']['delta_us'],3011)
        self.assertEqual(result['guard']['step'],3)
        self.assertEqual(result['reference_cycles']['previous_cycle_ticks'],6473)
        self.assertEqual(result['reference_cycles']['refused_cycle_ticks'],6018)
        self.assertEqual(result['outputs_off_verified'],1)
        self.assertFalse(result['cause_identified'])
        with self.assertRaises(ValueError):verify(text,30000,dropout=True,reentry=True)

    def test_retained_actual_vector_probe(self):
        from pathlib import Path
        from drv_priority_check import verify_dma
        root=Path(__file__).resolve().parents[1]/'captures'
        self.assertEqual(verify_dma((root/'dmapeer_priority01.txt').read_text())['trials'],3)

    def test_disabled_actual_vector_order_is_strict(self):
        from unittest.mock import patch
        from drv_priority_check import verify_dma
        trial=('DMAPRIORITYCASE mode=1 comp=64 dma=0 guard=0 sequence=123 expected=123 disabled=1\n'
               'DMAPRIORITYCASE mode=1 comp=64 dma=64 guard=0 sequence=132 expected=132 disabled=1\n'
               'DMAPRIORITYCASE mode=2 comp=64 dma=64 guard=0 sequence=213 expected=213 disabled=1\n'
               'DMAPRIORITYCHECK restored=1 gate_authority=0\n')
        good=trial*3+'FINALOFF\n'
        with patch('drv_priority_check.verify_off'):
            self.assertEqual(verify_dma(good)['trials'],3)
            for bad in [trial+'FINALOFF\n',good.replace('sequence=213','sequence=123'),
                        good.replace('guard=0','guard=64'),good.replace('restored=1','restored=0')]:
                with self.assertRaises(ValueError):verify_dma(bad)

    def test_dma_only_change_requires_explicit_expectation(self):
        baseline='COREPRIORITY comp=64 com=64 guard=0 dma=0'
        candidate='COREPRIORITY comp=64 com=64 guard=0 dma=64'
        verify_peer_priority(baseline,True)
        verify_peer_priority(candidate,True,dma_peer=True)
        with self.assertRaises(ValueError):verify_peer_priority(candidate,True)
        for bad in ['',baseline,candidate+'\n'+candidate,
                    candidate.replace('guard=0','guard=64'),
                    candidate.replace('com=64','com=128')]:
            with self.assertRaises(ValueError):verify_peer_priority(bad,True,dma_peer=True)


if __name__=='__main__':unittest.main()
