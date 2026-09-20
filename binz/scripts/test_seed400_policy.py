"""Compile actual seed policy against the existing host minz-core build."""
from pathlib import Path
import subprocess

if __name__=='__main__':
    root=Path(__file__).resolve().parents[1]
    build=root.parent/'minz/core/target/x86_64-pc-windows-msvc/debug'
    subprocess.run(['cargo','build','--manifest-path',str(root.parent/'minz/core/Cargo.toml'),
                    '--target','x86_64-pc-windows-msvc'],cwd=root,check=True)
    args=['rustc','--edition=2021','--test']
    for feature in ['bench-seed400','bench-range350','bench-seed-div12']:
        args+=['--cfg',f'feature="{feature}"']
    exe=root/'target/seed400_policy_test.exe'
    subprocess.run(args+['--extern',f'minz_core={build / "libminz_core.rlib"}',
                        '-L',f'dependency={build / "deps"}',str(root/'scripts/seed400_policy_test.rs'),
                        '-o',str(exe)],cwd=root,check=True)
    subprocess.run([str(exe)],cwd=root,check=True)
