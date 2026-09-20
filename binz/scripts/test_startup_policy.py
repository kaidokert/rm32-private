"""Run pure startup policies against real minz-core, without MCU vectors."""
from pathlib import Path
import subprocess

def main():
    root=Path(__file__).resolve().parents[1]
    core=root.parent/'minz/core'
    host='x86_64-pc-windows-msvc'
    subprocess.run(['cargo','build','--manifest-path',str(core/'Cargo.toml'),
                    '--target',host],cwd=root,check=True)
    build=core/'target'/host/'debug'
    executable=root/'target/startup_policy_test.exe'
    subprocess.run(['rustc','--edition=2021','--test',
                    '--extern',f'minz_core={build / "libminz_core.rlib"}',
                    '-L',f'dependency={build / "deps"}',
                    str(root/'scripts/startup_policy_test.rs'),'-o',str(executable)],
                   cwd=root,check=True)
    subprocess.run([str(executable)],cwd=root,check=True)

if __name__=='__main__':main()
