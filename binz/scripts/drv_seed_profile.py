"""Exact opt-in seed profile, separate from the running speed envelope."""
import re
EXPECTED='min_ticks=834 cycle_ticks=5000 individual_ticks=476 remaining_ticks=64 arm_max_us=16 shared_acquisition=1'
def profile(text):
    rows=re.findall(r'^SEEDPROFILE ([^\r\n]+)',text,re.M)
    if not rows:return None
    options={EXPECTED:(5000,476,834),
        EXPECTED.replace('min_ticks=834 cycle_ticks=5000','min_ticks=741 cycle_ticks=4445'):(4445,476,741),
        EXPECTED.replace('min_ticks=834 cycle_ticks=5000','min_ticks=667 cycle_ticks=4000'):(4000,476,667)}
    if len(rows)!=1 or rows[0] not in options:raise ValueError('invalid/duplicate seed profile')
    if not (any(f'RUNLIMIT cycle_min_us={floor} event_min_us=238 ' in text for floor in (2500,2223))
            or 'RUNLIMIT cycle_min_us=2223 event_min_us=100 ' in text):
        raise ValueError('seed400 requires cycle400 or cycle450 running profile')
    return options[rows[0]]

def seed400(text):
    return profile(text)==(5000,476,834)
def verify(text,required=False,seed450=False,seed500=False):
    if seed450 or seed500:
        rows=re.findall(r'^SEEDPROFILE ([^\r\n]+)',text,re.M)
        expected=EXPECTED.replace('min_ticks=834 cycle_ticks=5000','min_ticks=741 cycle_ticks=4445')
        if seed500: expected=EXPECTED.replace('min_ticks=834 cycle_ticks=5000','min_ticks=667 cycle_ticks=4000')
        if required or (seed450 and seed500) or rows!=[expected] or 'RUNLIMIT cycle_min_us=2223 event_min_us=100 ' not in text:
            raise ValueError('seed450 build/fixture mismatch')
        return
    selected=profile(text)
    if selected not in (None,(5000,476,834)) or bool(selected)!=required:
        raise ValueError('seed400 requires matching explicit fixture selection')
