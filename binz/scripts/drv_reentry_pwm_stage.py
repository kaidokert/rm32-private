"""Explicit candidate provenance, not an assertion of successful recovery."""
import re


def verify(text, required=False, guard=False, next_edge=False, expected_phase=False):
    modes = re.findall(r'^FOLLOWMODE ([^\r\n]+)', text, re.M)
    if modes != (['expected_phase_only=1'] if expected_phase else []) or (expected_phase and not next_edge):
        raise ValueError('expected-phase follow requires matching explicit fixture selection')
    rows = re.findall(r'^RECOVERPWMSTAGE ([^\r\n]+)', text, re.M)
    expected = ['enabled=1 before_sensing=1 physical_revalidate=1'] if required else []
    if rows != expected:
        raise ValueError('recovery PWM staging provenance mismatch')
    rows = re.findall(r'^RECOVERGUARDSTAGE ([^\r\n]+)', text, re.M)
    expected = ['enabled=1 fresh_admission=1'] if guard else []
    if rows != expected or (guard and not required):
        raise ValueError('recovery guard staging provenance mismatch')
    rows = re.findall(r'^FOLLOWEDGE ([^\r\n]+)', text, re.M)
    if not next_edge:
        if rows:
            raise ValueError('next-edge recovery requires explicit fixture selection')
        return
    if not guard or len(rows) != 1:
        raise ValueError('next-edge recovery requires guard staging and singleton evidence')
    match = re.fullmatch(r'result=(\d+) prior_step=(\d+) prior_tick=(\d+) step=(\d+) '
                         r'onset=(\d+) confirmed=(\d+) interval=(\d+) max_gap=(\d+) half_us=1', rows[0])
    if not match:
        raise ValueError('malformed next-edge evidence')
    result, prior_step, prior, step, onset, confirmed, interval, gap = map(int, match.groups())
    counts = re.findall(r'^FOLLOWREADS ([^\r\n]+)', text, re.M)
    reads = None
    if counts:
        count = re.fullmatch(r'count=(\d+)(?: captured=([0-6]))?(?: policy_ticks=(\d+))?(?: dma=(\d+))?', counts[0])
        if len(counts) != 1 or not count or int(count[1]) > 0xffffffff:
            raise ValueError('invalid follow sample count')
        reads = int(count[1])
        if count[3] is not None and int(count[3]) > 0xffffffff:
            raise ValueError('invalid policy timing')
        if count[4] is not None and (count[3] is None or int(count[4]) > 0xffffffff):
            raise ValueError('invalid policy DMA count')
        if count[2] is not None and result == 1 and int(count[2]) == 0:
            raise ValueError('qualified follow lacks capture checkpoint')
        if result == 5 and reads == 0:
            raise ValueError('sampling-gap refusal without a sample attempt')
    stages = re.findall(r'^FOLLOWSTAGE ([^\r\n]+)', text, re.M)
    stage = None
    if stages:
        pattern = r'stage=([0-6])'
        parsed = re.fullmatch(pattern, stages[0])
        if len(stages) != 1 or not parsed:
            raise ValueError('invalid next-edge checkpoint provenance')
        stage = int(parsed[1])
        if result == 1 and stage != 5:
            raise ValueError('qualified follow edge never reached final wait')
    if any(value > 0xffffffff for value in (prior, onset, confirmed, interval, gap)):
        raise ValueError('next-edge value outside u32')
    if result != 1:
        raise ValueError(f'next-edge recovery refused: result={result}, max_gap_ticks={gap}, stage={stage}, reads={reads}; not a recovery pass')
    if (not 1 <= prior_step <= 6 or step != prior_step % 6 + 1 or gap != 0
            or not 476 <= interval <= 2000 or (onset - prior) % 2**32 != interval
            or not (0 if 'FOLLOWPERSIST reads=12 sampled_dwell=0' in text else 40) <= (confirmed - onset) % 2**32 <= 240):
        raise ValueError('next-edge order/onset/confirmation evidence failed')
    seeds = re.findall(r'^RECOVERYACQ result=1 step=([1-6]) interval_ticks=\d+ elapsed_us=\d+ '
                       r'intervals=12 max_gap_ticks=\d+ disabled=0 gate_authority=0$', text, re.M)
    if seeds != [str(prior_step)]:
        raise ValueError('next-edge prior sector disagrees with qualified acquisition')
