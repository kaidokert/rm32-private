import tempfile
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit_linker


class AuditLinkerTests(unittest.TestCase):
    def test_sha256_file_is_content_exact(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "firmware"
            artifact.write_bytes(b"rm32\x00m0")
            self.assertEqual(
                audit_linker.sha256_file(artifact),
                "daaf314bda5ef536265f4dfa36c0d40377b2fdbd849b3a86b72ec6a38f025e47",
            )

    def test_transitive_soft_division_is_rejected(self):
        disassembly = """
08000000 <TIM6_DAC_LPTIM1>:
 8000000: f000 f800 bl 8000010 <tick>
08000010 <tick>:
 8000010: f000 f800 bl 8000020 <__aeabi_uidiv>
08000020 <__aeabi_uidiv>:
 8000020: 4770 bx lr
"""
        calls, edges = audit_linker.scan(disassembly)
        self.assertEqual(
            [(call["caller"], call["target"]) for call in audit_linker.reachable(
                calls, edges, "TIM6_DAC_LPTIM1"
            )],
            [("tick", "__aeabi_uidiv")],
        )

    def test_unreachable_foreground_division_is_reported_but_not_irq_violation(self):
        disassembly = """
08000000 <TIM14>:
 8000000: 4770 bx lr
08000010 <foreground>:
 8000010: f000 f800 bl 8000020 <__aeabi_uidiv>
"""
        calls, edges = audit_linker.scan(disassembly)
        self.assertEqual(len(calls), 1)
        self.assertEqual(audit_linker.reachable(calls, edges, "TIM14"), [])

    def test_rustc_response_file_is_expanded_without_eating_backslashes(self):
        with tempfile.TemporaryDirectory() as directory:
            response = Path(directory) / "linker-arguments"
            response.write_text('-flavor gnu -o "E:\\motor build\\firmware" -Tlink.x')
            args = audit_linker.expanded_args(["@" + str(response)])
        self.assertEqual(args[args.index("-o") + 1], "E:\\motor build\\firmware")


if __name__ == "__main__":
    unittest.main()
