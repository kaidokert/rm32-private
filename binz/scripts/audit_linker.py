"""Cargo target linker: link normally, then require a successful arithmetic audit."""
import os
from pathlib import Path
import subprocess
import sys

from drv_math_audit import audit,scan,reachable_forbidden

ISR_ROOTS=('DMA1_CHANNEL1','ADC_COMP','TIM16','TIM6_DAC_LPTIM1')


def main():
    args=sys.argv[1:]
    rustc=os.environ.get('RUSTC','rustc')
    root=Path(subprocess.check_output([rustc,'--print','sysroot'],text=True).strip())
    version=subprocess.check_output([rustc,'-vV'],text=True)
    host=next(line.split(': ',1)[1] for line in version.splitlines() if line.startswith('host: '))
    linker=root/'lib'/'rustlib'/host/'bin'/('rust-lld.exe' if os.name=='nt' else 'rust-lld')
    result=subprocess.run([str(linker),*args])
    if result.returncode:return result.returncode
    if '-o' not in args:raise RuntimeError('linked output not identified; audit refused')
    output=Path(args[args.index('-o')+1])
    report=audit(output)
    disassembly=Path(str(output)+'.math-audit.S').read_text(encoding='utf-8')
    _,edges=scan(disassembly)
    bad=[]
    for root_name in ISR_ROOTS:
        found=reachable_forbidden(report['calls'],edges,[root_name])
        print(f"M0 ISR math audit: {root_name} forbidden_reachable={len(found)}",file=sys.stderr)
        bad.extend((root_name,call) for call in found)
    print(f"M0 math audit: {len(report['calls'])} total helper call sites; report {output}.math-audit.json",file=sys.stderr)
    if bad:
        for root_name,call in bad:
            print(f"M0 ISR math violation: {root_name}: {call['caller']} -> {call['target']}",file=sys.stderr)
        return 1
    return 0


if __name__=='__main__':
    sys.exit(main())
