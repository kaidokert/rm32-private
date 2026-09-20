import unittest
import sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from scripts.drv_prepared_handoff import verify,verify_prevalidation,verify_direct,verify_setup_phase
from scripts.drv_driven_handoff import verify_peer_priority


class PreparedIdentity(unittest.TestCase):
    def test_setup_phase_identity(self):
        row='FOLLOWSETUP expected\r\n'
        verify_setup_phase(row,True);verify_setup_phase('',False)
        for text,enabled in [(row,False),('',True),(row.replace('expected','all'),True)]:
            with self.assertRaises(ValueError):verify_setup_phase(text,enabled)
    def test_direct_identity(self):
        row='FOLLOWDIRECT v1\r\n'
        verify_direct(row,True);verify_direct('',False)
        for text,enabled in [(row,False),('',True),(row.replace('v1','v2'),True)]:
            with self.assertRaises(ValueError):verify_direct(text,enabled)
    def test_prevalidation_identity(self):
        row='FOLLOWPREVALIDATE v1\r\n'
        verify_prevalidation(row,True);verify_prevalidation('',False)
        for text,enabled in [(row,False),('',True),(row.replace('v1','v2'),True)]:
            with self.assertRaises(ValueError):verify_prevalidation(text,enabled)
    def test_prepared_priority_requires_explicit_armed_configuration(self):
        text=('PREPAREDHANDOFF floor=64 arm=16 priority=0\r\n'
              'CORESEED armed=1 refusal_stop_code=8\r\n'
              'COREPRIORITY comp=64 com=0 guard=0 dma=64\r\n')
        verify_peer_priority(text,dma_peer=True,prepared_recovery=True)
        with self.assertRaises(ValueError):verify_peer_priority(text,dma_peer=True)
        for bad in [text.replace('com=0','com=64'),text.replace('guard=0','guard=64'),
                    text.replace('dma=64','dma=0'),text.replace('armed=1','armed=0'),
                    text.replace('PREPAREDHANDOFF','UNKNOWN')]:
            with self.assertRaises(ValueError):
                verify_peer_priority(bad,dma_peer=True,prepared_recovery=True)
        verify_peer_priority('COREPRIORITY comp=64 com=64 guard=0 dma=64\n',dma_peer=True)

    def test_configuration_must_be_explicit_and_exact(self):
        row='PREPAREDHANDOFF floor=64 arm=16 priority=0\r\n'
        verify(row,True);verify(row*2,True);verify('',False)
        for text,enabled in [('',True),(row,False),(row.replace('64','16'),True),
                             (row.replace('priority=0','priority=128'),True)]:
            with self.assertRaises(ValueError):verify(text,enabled)
