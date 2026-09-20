"""Verify a diagnostic-free-ISR representative hold; no inferred qZC/sigma."""
from pathlib import Path
import argparse,re,sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
import drv_capture

def one(path:Path, *, duty:int=150, duration:int=30_000_000):
    text=path.read_text()
    drv_capture.parse_dump(text)  # all compact-frame CRCs
    if text.count('FINALOFF\n')!=1: raise ValueError('unique FINALOFF')
    drv_capture.verify_off(text.split('FINALOFF\n')[1].encode())
    if len(re.findall(r'^LEANCORE r1 recorder=0 comp_max=0 com_max=0 control_progress=1 lean_irq=1\s*$',text,re.M))!=1:
        raise ValueError('not lean qualification image')
    if re.search(r'^(AVGDIAG|FIRSTACCEPT|COMPTAIL) ',text,re.M):
        raise ValueError('diagnostic recorder linked')
    if not re.search(r'^ACCEPTLOG n=0 drop=0 ',text,re.M) or re.search(r'^AE85 ',text,re.M):
        raise ValueError('accepted-event recorder not empty')
    p=re.search(r'^POWERPATH reason=(\d+) stop_us=(\d+) isr_max_us=(\d+) commit_max_us=(\d+) veto=(\d+) active=0 disabled=1$',text,re.M)
    t=re.search(r'^TRACKSTOP event_fault=(\d+) last_event_us=(\d+) sector=([1-6]) last_poll_us=(\d+) feedback_acquired_us=(\d+) ',text,re.M)
    c=re.search(r'^POWERCOMMITS applied=(\d+)$',text,re.M)
    x=re.search(r'^DRIVEX result=1 power_reason=(\d+) fresh_transfer=1$',text,re.M)
    coast=re.search(r'^COASTREF stop=(\d+) max_us=(\d+) gates_disabled=1 sense_mux_only=0 desync=0 polling=0 running=1 ',text,re.M)
    ack=re.search(rf'^D={duty:04X} I=[0-9A-F]{{4}}\s*$',text,re.M)
    if not all((p,t,c,x,coast,ack)):raise ValueError('missing qualification evidence')
    stop=int(p[2]);last=int(t[2]);poll=int(t[4]);feedback=int(t[5])
    if (int(p[1]),int(p[5]),int(t[1]),int(x[1]))!=(2,0,0,2):raise ValueError('non-deadline/faulted result')
    # At the shared deadline foreground may observe elapsed first (1), or the
    # powered owner may publish its deadline stop first (7). POWERPATH reason2
    # above is authoritative; no other coast reason is qualification-valid.
    if int(coast[1]) not in (1,7) or not duration<=stop<=duration+200 or int(coast[2])!=duration:
        raise ValueError('duration')
    if stop-last>1000 or stop-feedback>1000 or abs(stop-poll)>1000:raise ValueError('stale tracking/feedback')
    if int(c[1])<100_000:raise ValueError('insufficient control progress')
    return dict(path=path.name,stop_us=stop,commits=int(c[1]),last_event_age_us=stop-last,
                feedback_age_us=stop-feedback,outputs_off=True,qualification='representative_lean',
                qzc_sigma_available=False)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--duty',type=int,default=150)
    ap.add_argument('--duration-us',type=int,default=30_000_000)
    ap.add_argument('captures',nargs='+',type=Path)
    args=ap.parse_args()
    if not 40<=args.duty<=500:raise ValueError('duty outside50-percent campaign cap')
    rows=[one(p,duty=args.duty,duration=args.duration_us) for p in args.captures]
    if len(rows)<3:raise ValueError('representative cohort requires three retained runs')
    print(rows)
if __name__=='__main__':main()
