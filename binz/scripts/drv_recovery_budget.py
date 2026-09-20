"""Planning arithmetic, not admission authority, WCET or hardware qualification.

Reference uses half-microsecond ticks and advance=16. This models a constant
speed seed only; a real twelve-interval seed must independently qualify.
"""
import argparse
import json

def budget(ehz, observed_age_ticks=160):
    if not isinstance(ehz,int) or not 1<=ehz<=2000 or observed_age_ticks<0:
        raise ValueError('invalid planning input')
    # Conservative ceil conversion; do not infer actual measured rotor speed.
    interval=(2_000_000+6*ehz-1)//(6*ehz)
    wait=(interval>>1)-((interval*16)>>6)
    age_limit=wait-64
    return dict(ehz=ehz,synthetic_interval_ticks=interval,wait_ticks=wait,
                age_limit_ticks=age_limit,
                observed_age_ticks=observed_age_ticks,
                required_saving_ticks=max(0,observed_age_ticks-age_limit),
                current_seed_profile_admits_constant_train=interval>=834 and 6*interval>=5000,
                tick_us=0.5,hardware_qualified=False,changes_admission=False)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ehz',type=int,nargs='+')
    parser.add_argument('--age-ticks',type=int,default=160)
    args=parser.parse_args()
    print(json.dumps([budget(f,args.age_ticks) for f in args.ehz],indent=2))
