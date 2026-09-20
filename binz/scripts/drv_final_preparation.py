"""Provenance only: consuming preparation is never a recovery/lock verdict."""
import re

def verify(text,required=False,*,used=None):
    rows=re.findall(r'^FINALPREP\b[^\r\n]*',text,re.M)
    if not rows:
        if required:raise ValueError('missing final-edge preparation provenance')
        return None
    if not required:raise ValueError('final-edge preparation requires explicit fixture selection')
    if len(rows)!=1:raise ValueError('duplicate final-edge preparation provenance')
    match=re.fullmatch(r'FINALPREP used=([01]) interval=11 output_authority=0 final_edge_checks=1',rows[0])
    if not match:raise ValueError('invalid final-edge preparation provenance')
    consumed=bool(int(match[1]))
    if used is not None and consumed!=used:
        raise ValueError('final-edge preparation consumption disagrees with verified campaign')
    return consumed
