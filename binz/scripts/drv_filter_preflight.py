"""Fail-fast disabled diagnostics for the filter observer candidate. No motor run."""
import argparse
from pathlib import Path
import subprocess
import sys

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--prefix',type=Path,required=True)
    ap.add_argument('--raw',action='store_true')
    ap.add_argument('--period-ticks',type=int,choices=[6400,3200,2666],default=2666)
    args=ap.parse_args()
    scripts=Path(__file__).resolve().parent
    checks=[('pulse','drv_filter_check.py',[]),
            ('atomic','drv_atomic_check.py',[]),
            ('roles','drv_role_check.py',['--period-ticks',str(args.period_ticks),'--duty','62']),
            ('cpu','drv_cpu_check.py',['--union']),
            ('archive','drv_archive_check.py',[])]
    if args.raw:checks.extend([('rawphase','drv_adc_phase_check.py',['--raw-capture']),('rawadc','drv_raw_adc_check.py',[])])
    for suffix,script,flags in checks:
        out=Path(str(args.prefix)+'_'+suffix+'.txt')
        subprocess.run([sys.executable,str(scripts/script),'--out',str(out),*flags],check=True)
    print('All disabled filter preflight gates passed. No motor command issued.')

if __name__=='__main__':main()
