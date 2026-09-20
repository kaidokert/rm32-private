"""Candidate configuration identity only; not proof of executed recovery."""
import re

def verify_persistence(text,enabled):
    rows=re.findall(r'^FOLLOWPERSIST .*$',text.replace('\r',''),re.M)
    if bool(rows)!=enabled or any(row!='FOLLOWPERSIST reads=12 sampled_dwell=0' for row in rows):
        raise ValueError('follow persistence build/fixture mismatch')

def verify_prevalidation(text,enabled):
    rows=re.findall(r'^FOLLOWPREVALIDATE .*$',text.replace('\r',''),re.M)
    if bool(rows)!=enabled or any(row!='FOLLOWPREVALIDATE v1' for row in rows):
        raise ValueError('follow-prevalidation build/fixture mismatch')

def verify_direct(text,enabled):
    rows=re.findall(r'^FOLLOWDIRECT .*$',text.replace('\r',''),re.M)
    if bool(rows)!=enabled or any(row!='FOLLOWDIRECT v1' for row in rows):
        raise ValueError('direct-follow build/fixture mismatch')

def verify_setup_phase(text,enabled):
    rows=re.findall(r'^FOLLOWSETUP .*$',text.replace('\r',''),re.M)
    if bool(rows)!=enabled or any(row!='FOLLOWSETUP expected' for row in rows):
        raise ValueError('setup-phase build/fixture mismatch')


def verify(text, enabled):
    rows=re.findall(r'^PREPAREDHANDOFF .*$',text.replace('\r',''),re.M)
    if bool(rows)!=enabled:
        raise ValueError('prepared-handoff build/fixture flag mismatch')
    if any(row!='PREPAREDHANDOFF floor=64 arm=16 priority=0' for row in rows):
        raise ValueError('unknown prepared-handoff configuration')
