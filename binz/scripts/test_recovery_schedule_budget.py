"""Conditional scheduling arithmetic, NOT measured WCET or firmware admission.

The inputs describe a pending first DMA scan. No motor authority is granted.
Keep first publication, DMA lease and first COM as separate deadlines.
"""
import unittest


def pending_budget(*, baseline_age, pending_at, defer, dma_cost,
                   com_preemption=0, next_scan_after=201):
    publication = pending_at + defer + dma_cost + com_preemption
    return {
        'publication_at': publication,
        'freshness_slack': 1000 - baseline_age - publication,
        # Conservative completion before the next scan, not just ISR entry.
        'scan_slack': next_scan_after - defer - dma_cost - com_preemption,
    }


class RecoveryScheduleBudget(unittest.TestCase):
    def test_masking_policy_only_does_not_remove_total_delay(self):
        # A pending IRQ serviced immediately on unmask still precedes arm.
        policy, later, dma = 16, 38, 42
        self.assertEqual(policy + dma + later, dma + policy + later)
        self.assertGreater(20 + policy + dma + later + 32, 109.5)

    def test_com_preemption_can_break_first_publication(self):
        # Illustrative phase, NOT a measured arrival/WCET combination.
        normal = pending_budget(baseline_age=832, pending_at=53,
                                defer=60, dma_cost=42)
        peer = pending_budget(baseline_age=832, pending_at=53,
                              defer=60, dma_cost=42, com_preemption=45)
        self.assertEqual(normal['freshness_slack'], 13)
        self.assertEqual(peer['freshness_slack'], -32)
        self.assertGreater(peer['scan_slack'], 0)

    def test_freshness_and_scan_deadlines_are_independent(self):
        result = pending_budget(baseline_age=0, pending_at=0,
                                defer=190, dma_cost=42)
        self.assertGreater(result['freshness_slack'], 0)
        self.assertLess(result['scan_slack'], 0)

    def test_exact_freshness_boundary(self):
        self.assertEqual(pending_budget(baseline_age=832,pending_at=53,
                                        defer=73,dma_cost=42)['freshness_slack'],0)
        self.assertEqual(pending_budget(baseline_age=832,pending_at=53,
                                        defer=74,dma_cost=42)['freshness_slack'],-1)


if __name__ == '__main__':
    unittest.main()
