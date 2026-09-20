"""Explicit report-only fast-cycle policy; never infer it from a successful run."""
import re

def decode(text):
    rows=re.findall(r'^FASTCYCLE ([^\r\n]+)',text,re.M)
    if not rows:return None
    match=re.fullmatch(r'r1 (\d+) (\d+)',rows[0])
    if len(rows)!=1 or not match:raise ValueError('invalid/duplicate fast-cycle policy')
    count,minimum=map(int,match.groups())
    if count>0xffffffff or (count==0 and minimum!=0) or (count>0 and not 0<minimum<2223):
        raise ValueError('invalid fast-cycle statistics')
    return dict(report_only=True,threshold_us=2223,count=count,min_us=minimum,segment_only=True)

def verify(text,required=False):
    report=decode(text)
    if bool(report)!=required:raise ValueError('fast-cycle report policy requires explicit fixture selection')
    return report
