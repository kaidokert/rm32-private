"""Run disabled-only TIM3/TIM6 checks; never issues a motor-start command."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text,real=False,comp=False,cancel=False,phased=False):
    rows=re.findall(r'^DRIVENCHECK reason=(\d+) stop_us=(\d+) ticks=(\d+) commands=(\d+) tick_max_us=(\d+) sector_max_us=(\d+) poststop_refused=1 disabled=1 synthetic_feedback='+str(int(not real))+r' gate_authority=0 sector_period_us='+('0' if phased else '833')+'$',text,re.M)
    if len(rows)!=3: raise ValueError('three complete disabled timer checks required')
    for row in rows:
        reason,stop,ticks,commands,tick_max,sector_max=map(int,row)
        timing=(reason==9 and 5000<=stop<=5200 and 99<=ticks<=104 and 5<=commands<=6) if cancel else (
            reason==2 and 20000<=stop<=20200 and (398 if comp else 198)<=ticks<=(401 if comp else 201) and 23<=commands<=24)
        if not (timing and tick_max<=50 and sector_max<=50):
            raise ValueError('disabled timer timing/stop contract failed: '+str(row))
    marker=text.rfind('FINALOFF\n')
    if marker<0: raise ValueError('missing final-off marker')
    verify_off(text[marker:].encode())
    if real:
        adc=re.findall(r'^DRIVENADC scans=(\d+) max_age_us=(\d+) peak_abs_raw=(\d+) bus_min_mv=(\d+) scan_start_aged=1 gates_disabled=1$',text,re.M)
        if len(adc)!=3: raise ValueError('three real feedback summaries required')
        for row in adc:
            n,age,peak,bus=map(int,row)
            if not ((20 if cancel else 50)<=n<=500 and 1<=age<=1000 and peak<=1200 and bus>=8400):
                raise ValueError('real feedback acquisition contract failed')
    if comp:
        obs=re.findall(r'^DRIVENCOMP reads=(\d+) gap_max_us=(\d+) bracket_max_us=(\d+) steps_mask=63 candidates=(\d+) missed=(\d+) reject_none=0 reject_epoch=0 reject_blank=(\d+) reject_bracket=0 reject_gap=0 period_us=50 synthetic_sectors=1 gate_authority=0$',text,re.M)
        if len(obs)!=3: raise ValueError('three complete comparator scheduling summaries required')
        for row in obs:
            reads,gap,bracket,candidates,missed,blank=map(int,row)
            if not ((99<=reads<=104 and 15<=blank<=35) if cancel else (397<=reads<=400 and 50<=blank<=120)):
                raise ValueError('comparator sampling count contract failed')
            if not (1<=gap<=100 and bracket<=2 and candidates<=25 and missed<=24):
                raise ValueError('comparator scheduling contract failed')
    if cancel and len(re.findall(r'^DRIVENCANCEL shared_gates_off=1 late_callbacks=2 commands_unchanged=1 timers_off=1 owner_off=1 disabled=1 gate_authority=0$',text,re.M))!=3:
        raise ValueError('shared stop or late-callback refusal failed')
    if phased:
        phase_rows=re.findall(r'^DRIVENPHASE theta=305419896 rate=858993 first_us=(\d+) next_deadline_us=(\d+) late_max_us=(\d+) synthetic_phase=1 gate_authority=0$',text,re.M)
        if len(phase_rows)!=3: raise ValueError('missing phase schedule evidence')
        source=(Path(__file__).resolve().parents[1]/'examples/support/sine_table.rs').read_text()
        lut=list(map(int,re.findall(r'\d+',source.split('= [',1)[1].split('];',1)[0])))
        pairs={(0,1):1,(2,1):2,(2,0):3,(1,0):4,(1,2):5,(0,2):6}
        def sector(us):
            i=((305419896+858993*us)&0xffffffff)>>24
            values=[lut[(i+p*85)&255] for p in range(3)]
            source=max(range(3),key=lambda p:(values[p],p))
            sink=min(range(3),key=lambda p:(values[p],p))
            return pairs[(source,sink)]
        boundaries=[];previous=sector(0)
        for us in range(1,22001):
            current=sector(us)
            if current!=previous: boundaries.append(us);previous=current
        first=boundaries[0];next_due=next(t for t in boundaries if t>=20000)
        for row in phase_rows:
            f,n,late=map(int,row)
            if (f,n)!=(first,next_due) or late>50: raise ValueError('phase continuity or dispatch lateness failed')
    return rows

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--real-adc',action='store_true',help='wake CSA and read real feedback; all gates remain disabled')
    ap.add_argument('--comp',action='store_true',help='real ADC plus 50us comparator sampling; synthetic sectors, NO PWM')
    ap.add_argument('--cancel',action='store_true',help='test shared gates_off cancellation at5ms and late callbacks; NO PWM')
    ap.add_argument('--phase',action='store_true',help='test variable sector periods from synthetic sine phase; NO PWM')
    args=ap.parse_args()
    if args.phase and args.cancel: ap.error('--phase and --cancel are separate experiments')
    # Reserve destination before opening hardware; preserve partial evidence.
    with args.out.open('xb') as log:
        data=bytearray()
        with serial.Serial('COM41',115200,timeout=.05) as port:
            try:
                read_available(port,.2)
                for cmd in ['off','p','i']:
                    send_line(port,cmd);data.extend(read_available(port,.2))
                verify_off(bytes(data))
                for _ in range(3):
                    send_line(port,'drivenphase' if args.phase else ('drivenstop' if args.cancel else ('drivencomp' if args.comp else ('drivenadc' if args.real_adc else 'drivencheck'))));data.extend(read_available(port,.3))
            finally:
                data.extend(b'FINALOFF\n')
                for cmd in ['off','p','i','stack']:
                    send_line(port,cmd);data.extend(read_available(port,.2))
                log.write(data)
        text=data.decode('ascii').replace('\r','');print(text)
        print('verified',len(verify(text,args.real_adc or args.comp or args.cancel or args.phase,args.comp or args.cancel or args.phase,args.cancel,args.phase)),'disabled timer trials; no motor authority')

if __name__=='__main__': main()
