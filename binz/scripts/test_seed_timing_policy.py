"""Compile/run the standalone Rust policy harness against the real host core."""
from pathlib import Path
import subprocess
import argparse

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--next-edge',action='store_true')
    parser.add_argument('--seed400',action='store_true')
    parser.add_argument('--seed450',action='store_true')
    parser.add_argument('--seed500',action='store_true')
    args=parser.parse_args()
    root=Path(__file__).resolve().parents[1]
    core=root.parent/'minz/core'
    host='x86_64-pc-windows-msvc'
    subprocess.run(['cargo','build','--manifest-path',str(core/'Cargo.toml'),
                    '--target',host],cwd=root,check=True)
    build=core/'target'/host/'debug'
    executable=root/'target/seed_timing_reanchor_test.exe'
    subprocess.run(['rustc','--edition=2021','--test',
                    *(['--cfg','feature="bench-reentry-next-edge"'] if args.next_edge else []),
                    *(['--cfg','feature="bench-seed400"'] if args.seed400 else []),
                    *(['--cfg','feature="bench-seed450"','--cfg','feature="bench-seed400"'] if args.seed450 else []),
                    *(['--cfg','feature="bench-seed500"','--cfg','feature="bench-seed450"','--cfg','feature="bench-seed400"'] if args.seed500 else []),
                    '--cfg','feature="bench-seed-timing-reanchor"',
                    '--cfg','feature="bench-range350"',
                    '--extern',f'minz_core={build / "libminz_core.rlib"}',
                    '-L',f'dependency={build / "deps"}',
                    str(root/'scripts/seed_timing_reanchor_test.rs'),
                    '-o',str(executable)],cwd=root,check=True)
    subprocess.run([str(executable)],cwd=root,check=True)
