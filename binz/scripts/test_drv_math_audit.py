import unittest
from drv_math_audit import category,scan,reachable_forbidden


class MathAuditTests(unittest.TestCase):
    def test_seed_math_provenance(self):
        from drv_driven_handoff import verify_seed_math
        line='SEEDMATH div12_bound=24000 exact=1 fallback=1 qualification_unchanged=1'
        verify_seed_math(line,required=True)
        verify_seed_math('')
        for bad in ['',line+'\n'+line,line.replace('24000','65535'),line+' extra=1']:
            with self.assertRaises(ValueError):verify_seed_math(bad,required=True)

    def test_categories(self):
        for name in ['__aeabi_uidiv','__aeabi_idivmod','__aeabi_uldivmod','__udivdi3']:
            self.assertEqual(category(name),'division/remainder helper')
        for name in ['__multi3','__udivti3','__ashlti3']:
            self.assertEqual(category(name),'128-bit helper')
        for name in ['__aeabi_lmul','__muldi3','__lshrdi3']:
            self.assertEqual(category(name),'64-bit helper')
        for name in ['__aeabi_dmul','__aeabi_fadd','__adddf3','__floatundidf']:
            self.assertEqual(category(name),'software floating-point helper')
        self.assertIsNone(category('__aeabi_memcpy4'))

    def test_source_interleaved_calls_and_tail_branches(self):
        text='''08000000 <motor::edge>:
    let n = x / 12;
 8000002: f00d f8da bl 801843e <__aeabi_uidiv>
 8000006: e001 b.n 8000010 <__aeabi_lmul>
 8000008: 4798 blx r3
08000010 <__aeabi_lmul>:
 8000010: e7fe b.n 8000010 <__aeabi_lmul+0x0>
'''
        calls,_=scan(text)
        self.assertEqual(len(calls),2)
        self.assertEqual(calls[0]['caller'],'motor::edge')
        self.assertEqual(calls[1]['kind'],'64-bit helper')

    def test_reachable_direct_call_chain_finds_nested_helper(self):
        text='''08000000 <DMA1_CHANNEL1>:
 8000000: f000 f800 bl 8000010 <feedback>
08000010 <feedback>:
 8000010: f000 f800 bl 8000020 <convert>
08000020 <convert>:
 8000020: f000 f800 bl 8000030 <__aeabi_uidiv>
08000030 <__aeabi_uidiv>:
 8000030: 4770 bx lr
'''
        calls,edges=scan(text)
        bad=reachable_forbidden(calls,edges,['DMA1_CHANNEL1'])
        self.assertEqual([(c['caller'],c['target']) for c in bad],
                         [('convert','__aeabi_uidiv')])
