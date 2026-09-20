import unittest
from pathlib import Path
from drv_persistence_codegen import inspect,disassemble


class PersistenceCodegenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text=disassemble(Path(__file__).resolve().parents[1]/'captures/reference/direct_bc876/shell-pwm.elf')

    def test_frozen_static_loop_not_dynamic_fallback(self):
        r=inspect(self.text)
        self.assertEqual(r['successful_iteration_instructions'],15)
        self.assertEqual(r['comparator_load'],'0x8001cd2')
        self.assertFalse(r['cycles_measured'])
        self.assertFalse(r['preemption_excluded'])

    def test_unknown_signal_address_refuses(self):
        with self.assertRaises(ValueError):inspect(self.text.replace('0x40010204','0x40010200'))

    def test_call_in_loop_refuses(self):
        altered=self.text.replace('eors\tr1, r2','bl\t8001000 <unknown>')
        self.assertNotEqual(altered,self.text)
        with self.assertRaises(ValueError):inspect(altered)
