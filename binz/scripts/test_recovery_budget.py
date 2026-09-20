"""Compile the actual reference wait arithmetic for the recovery budget audit."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]


def function(source,name):
    start=source.index('pub fn '+name+'(')
    brace=source.index('{',start)
    depth=1
    end=brace+1
    while depth:
        depth+=(source[end]=='{')-(source[end]=='}')
        end+=1
    return source[start:end]


class RecoveryBudget(unittest.TestCase):
    def test_actual_reference_arithmetic(self):
        source=(ROOT.parent/'minz/core/src/am32.rs').read_text()
        harness=function(source,'advance_of')+'\n'+function(source,'wait_time')+r'''
#[test] fn observed_recovery_exact_boundary() {
    let wait=wait_time(871,advance_of(871,16));
    assert_eq!(wait,218); // half-us ticks
    assert_eq!(wait-154,64); // measured77us age, exactly32us remaining
}
#[test] fn confirmation_plus_arm_floor_exceeds_high_speed_window() {
    for (interval,expected_wait) in [(360,90),(333,83)] {
        let wait=wait_time(interval,advance_of(interval,16));
        assert_eq!(wait,expected_wait);
        assert!(wait<40+64); // existing20us confirmation +32us remaining
    }
}
#[test] fn changing_seed_acceptance_does_not_create_time() {
    let wait=wait_time(333,advance_of(333,16));
    assert_eq!(wait.checked_sub(154),None);
    assert!(wait-40<64); // evenzero post-confirmation setup cannot fit
}
'''
        with tempfile.TemporaryDirectory() as temp:
            src=Path(temp)/'budget.rs';exe=Path(temp)/'budget.exe'
            src.write_text(harness)
            subprocess.run(['rustc','--test',str(src),'-o',str(exe)],check=True)
            subprocess.run([str(exe)],check=True)


if __name__=='__main__':unittest.main()
