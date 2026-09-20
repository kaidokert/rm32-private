"""Build then run current guard policy; never execute a stale test binary."""
from pathlib import Path
import subprocess
import argparse

if __name__ == '__main__':
    root=Path(__file__).resolve().parents[1]
    parser=argparse.ArgumentParser()
    parser.add_argument('--cycle450',action='store_true')
    parser.add_argument('--fast-cycle-report',action='store_true')
    parser.add_argument('--event100',action='store_true')
    parser.add_argument('--reentry-guard-stage',action='store_true')
    parser.add_argument('--next-edge',action='store_true')
    args=parser.parse_args()
    exe=root/'target/guard_install_policy_test.exe'
    subprocess.run(['rustc','--edition=2021','--test',*(['--cfg','feature="bench-reentry-next-edge"'] if args.next_edge else []),*(['--cfg','feature="bench-reentry-guard-stage"'] if args.reentry_guard_stage else []),*(['--cfg','feature="bench-event100"'] if args.event100 else []),'--cfg',
                    'feature="bench-guard-install"',
                    str(root/'scripts'/('fast_cycle_report_test.rs' if args.fast_cycle_report else 'cycle450_guard_test.rs' if args.cycle450 else 'cycle400_guard_test.rs')),'-o',str(exe)],check=True)
    subprocess.run([str(exe)],check=True)
