"""Check the actual before/after ISR helper regression, not CPU headroom."""
import json
from pathlib import Path
import unittest
from drv_math_audit import forbidden_calls

ROOT=Path(__file__).resolve().parents[1]

class Enforcement(unittest.TestCase):
    def test_saved_before_after_commutation(self):
        old=json.loads((ROOT/'captures/reference/leanirq_706/shell-pwm.math-audit.json').read_text())
        new=json.loads((ROOT/'captures/reference/notrace_707/shell-pwm.math-audit.json').read_text())
        callers={c['caller'] for c in old['calls'] if 'TIM16' in c['caller']}
        self.assertTrue(callers)
        bad=forbidden_calls(old,callers)
        self.assertTrue(any(c['target']=='__aeabi_uidiv' for c in bad))
        self.assertEqual(forbidden_calls(new,callers),[])

    def test_exact_names_not_substring_and_all_categories(self):
        calls=[{'caller':'TIM16','target':name} for name in
               ['__aeabi_uidiv','__aeabi_lmul','__aeabi_dmul']]
        report={'calls':calls+[{'caller':'foreground_TIM16_report','target':'__aeabi_uidiv'}]}
        self.assertEqual(forbidden_calls(report,{'TIM16'}),calls)
        self.assertEqual(forbidden_calls(report,set()),[])

if __name__=='__main__':unittest.main()
