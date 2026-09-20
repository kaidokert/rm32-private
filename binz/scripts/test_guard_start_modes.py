"""Source contract + exhaustive mode algebra; not PAC/hardware execution."""
from pathlib import Path
import itertools
import unittest

class StartModes(unittest.TestCase):
    def test_private_call_modes_and_original_limits(self):
        text=(Path(__file__).resolve().parents[1]/'examples/support/powered_timer.rs').read_text()
        self.assertIn('start_inner::<false,false>(elapsed,step,sample,0,20_000,UNUSED_LIMITS)',text)
        self.assertIn('start_inner::<true,false>(elapsed,step,sample,age,window_us,UNUSED_LIMITS)',text)
        self.assertIn('start_inner::<true,true>(limits.elapsed,step,sample,age,limits.segment,limits)',text)
        self.assertEqual(text.count('start_inner::<'),3)
        self.assertIn('if adopted && (!awake || REENTRY) {return false;}',text)
        self.assertIn('if staged && (!awake || !REENTRY || adopted) {return false;}',text)
        self.assertIn('if REENTRY {Some(limits.elapsed)}else{powered_guard::reserved_elapsed(elapsed)}',text)
        self.assertIn('if REENTRY {(limits.campaign,limits.segment)}',text)

    def test_mode_refusal_algebra(self):
        for awake,adopted,staged,reentry in itertools.product([False,True],repeat=4):
            resume=object() if reentry else None
            old=(adopted and (not awake or resume is not None)) or (staged and (not awake or resume is None or adopted))
            new=(adopted and (not awake or reentry)) or (staged and (not awake or not reentry or adopted))
            self.assertEqual(old,new)
