"""Small machine-code fixtures for entry reachability, not a full decoder test."""
import unittest
from isr_audit import parse, reachable


class InstructionReachability(unittest.TestCase):
    def test_hex_spelled_mnemonic_is_not_machine_code(self):
        f = parse("080000f8 <ADC_COMP>:\n 80000f8: af00 add r7, sp, #0\n 80000fa: 4770 bx lr\n")
        self.assertEqual(f["ADC_COMP"].cfg[0][1], "add")
        self.assertFalse(f["ADC_COMP"].indirect)

    def test_padding_after_return_is_not_a_call(self):
        f = parse("""080000f8 <ADC_COMP>:
 80000f8: b580 push {r7, lr}
 80000fa: f001 fcf4 bl 8001ae8 <body>
 80000fe: bd80 pop {r7, pc}
 8000100: d4d4 bmi.n 80000ac <__INTERRUPTS+0x6c>
08001ae8 <body>:
 8001ae8: 4770 bx lr
""")
        self.assertEqual(reachable(f, "ADC_COMP"), ({"ADC_COMP", "body"}, []))
        self.assertEqual(f["ADC_COMP"].insns, 3)

    def test_branch_reaches_callee_after_first_return(self):
        f = parse("""080000f8 <ADC_COMP>:
 80000f8: d001 beq.n 80000fe <ADC_COMP+0x6>
 80000fa: 4770 bx lr
 80000fc: d4d4 bmi.n 80000ac <__INTERRUPTS+0x6c>
 80000fe: f001 fcf4 bl 8001ae8 <__aeabi_uidiv>
 8000102: 4770 bx lr
""")
        self.assertEqual(f["ADC_COMP"].calls, {"__aeabi_uidiv"})
        self.assertIn("__aeabi_uidiv", reachable(f, "ADC_COMP")[1])

    def test_reachable_indirect_tail_is_not_a_return(self):
        for opcode in ["bx", "blx"]:
            for register in ["r3", "ip", "r12"]:
                f = parse(f"080000f8 <ADC_COMP>:\n 80000f8: 4718 {opcode} {register}\n")
                self.assertTrue(f["ADC_COMP"].indirect)

    def test_unannotated_numeric_call_is_not_ignored(self):
        f = parse("080000f8 <ADC_COMP>:\n 80000f8: f001 fcf4 bl 8001aec\n")
        self.assertTrue(f["ADC_COMP"].indirect)

    def test_other_pc_writes_and_unknown_instructions_fail_closed(self):
        for operation in ["mov pc, r3", "add pc, r3", "ldr pc, [r3]",
                          "ldmia r3!, {r0, pc}", "tbb [r0, r1]", "svc 0", "wfi"]:
            f = parse(f"080000f8 <ADC_COMP>:\n 80000f8: 469f {operation}\n 80000fa: 4770 bx lr\n")
            self.assertTrue(f["ADC_COMP"].indirect, operation)

    def test_unannotated_tail_transfer_fails_but_internal_branch_survives(self):
        for header, expected in [("08000100 <__aeabi_uidiv>:\n", True), ("", False)]:
            f = parse(f"""080000f8 <ADC_COMP>:
 80000f8: e002 b.n 8000100
{header} 8000100: 4770 bx lr
""")
            self.assertEqual(f["ADC_COMP"].indirect, expected)

    def test_same_symbol_interior_call_is_unresolved(self):
        f = parse("""080000f8 <ADC_COMP>:
 80000f8: f001 fcf4 bl 8000100 <ADC_COMP+0x8>
 80000fc: 4770 bx lr
 8000100: f001 fcf4 bl 8003000 <__aeabi_uidiv>
""")
        self.assertIn("ADC_COMP+0x8", reachable(f, "ADC_COMP")[1])

    def test_external_interior_entry_is_unresolved_not_silently_pruned(self):
        f = parse("""080000f8 <ADC_COMP>:
 80000f8: f001 fcf4 bl 8001aec <body+0x4>
 80000fc: 4770 bx lr
08001ae8 <body>:
 8001ae8: 4770 bx lr
 8001aec: f001 fcf4 bl 8003000 <__aeabi_uidiv>
""")
        self.assertIn("body+0x4", reachable(f, "ADC_COMP")[1])

    def test_dead_loop_removed_live_loop_retained(self):
        for prefix, expected in [(" 80000f8: 4770 bx lr", False),
                                 (" 80000f8: e000 b.n 80000fc <ADC_COMP+0x4>", True)]:
            f = parse(f"""080000f8 <ADC_COMP>:
{prefix}
 80000fa: d4d4 bmi.n 80000ac <__INTERRUPTS+0x6c>
 80000fc: e7fe b.n 80000fc <ADC_COMP+0x4>
""")
            self.assertEqual(f["ADC_COMP"].has_backward_branch, expected)


if __name__ == "__main__":
    unittest.main()
