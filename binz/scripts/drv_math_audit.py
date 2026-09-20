"""Read-only ELF inspection; generated reports live alongside the linked artifact.

Helper calls are potential costs, not an exclusive list of wide arithmetic.
Inline add/shift/multiply sequences and indirect calls require manual review.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def category(symbol):
    if re.search(r'__(?:u?div|u?mod|mul|ashl|ashr|lshr|add|sub|neg|cmp|ucmp|multi|divmod).*ti[234]$',symbol):
        return '128-bit helper'
    if re.search(r'__aeabi_.*div|__(?:u?div|u?mod|divmod|udivmod)(?:si|di|ti)[234]$',symbol):
        return 'division/remainder helper'
    if re.search(r'__aeabi_(?:lmul|llsl|llsr|lasr|lcmp|ulcmp)|__(?:mul|ashl|ashr|lshr|add|sub|neg|cmp|ucmp)di[234]$',symbol):
        return '64-bit helper'
    if re.search(r'__aeabi_(?:[df]|[ui]2[df]|[lu]+2[df])|__(?:add|sub|mul|div|extend|trunc|fix|float|eq|ne|lt|le|gt|ge|unord).*(?:sf|df|tf)',symbol):
        return 'software floating-point helper'
    return None


def scan(disassembly):
    caller=None
    calls=[]
    edges={}
    for line in disassembly.splitlines():
        header=re.match(r'^([0-9a-fA-F]+) <(.+)>:$',line)
        if header:caller=header[2];continue
        insn=re.match(r'^\s*([0-9a-fA-F]+):\s+(?:[0-9a-fA-F]{4,8}\s+)+([a-z][a-z0-9.]*)\s+(.+)$',line)
        if not insn or insn[2] not in ('bl','blx','b','b.w','b.n'):continue
        target=re.search(r'<([^>]+)>',insn[3])
        if not target:continue
        symbol=re.sub(r'\+0x[0-9a-fA-F]+$','',target[1])
        if caller is not None and caller!=symbol:
            edges.setdefault(caller,set()).add(symbol)
        kind=category(symbol)
        if kind and caller!=symbol:
            calls.append(dict(address='0x'+insn[1],caller=caller,target=symbol,kind=kind))
    return calls,edges


def forbidden_calls(report, callers):
    """Exact emitted callers only; does not claim a transitive ISR audit."""
    return [call for call in report['calls'] if call['caller'] in callers]


def reachable_forbidden(calls,edges,roots):
    """Find helper calls reachable through emitted direct-call edges."""
    reached=set(roots)
    pending=list(roots)
    while pending:
        caller=pending.pop()
        for target in edges.get(caller,()):
            if target not in reached:
                reached.add(target);pending.append(target)
    return [call for call in calls if call['caller'] in reached]


def audit(elf):
    elf=Path(elf)
    data=elf.read_bytes()
    if data[:4]!=b'\x7fELF':raise ValueError('expected linked ELF')
    command=['arm-none-eabi-objdump','-d','-S','-C',str(elf)]
    output=subprocess.check_output(command,text=True,encoding='utf-8',errors='replace')
    allow_path=Path(__file__).with_name('math_audit_allowlist.json')
    allows=json.loads(allow_path.read_text(encoding='utf-8'))
    calls,edges=scan(output)
    for call in calls:
        call['review_note']=next((a['reason'] for a in allows
            if a['caller']==call['caller'] and a['target']==call['target']),None)
    report=dict(elf=str(elf),sha256=hashlib.sha256(data).hexdigest(),command=command,
                calls=calls,unreviewed=sum(c['review_note'] is None for c in calls),
                direct_call_edges=sum(map(len,edges.values())),advisory=True,
                inline_wide_math_excluded=True,indirect_calls_excluded=True)
    Path(str(elf)+'.math-audit.S').write_text(output,encoding='utf-8')
    Path(str(elf)+'.math-audit.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    return report


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf',type=Path)
    parser.add_argument('--forbid-caller',action='append',default=[],
                        help='fail on any arithmetic helper in this exact emitted caller; repeatable')
    parser.add_argument('--forbid-reachable-from',action='append',default=[],
                        help='fail on helpers reachable by emitted direct calls from this root')
    args=parser.parse_args()
    result=audit(args.elf)
    print(json.dumps(result,indent=2))
    if forbidden_calls(result,args.forbid_caller):
        parser.exit(1,'forbidden arithmetic helper in emitted caller\n')
    if args.forbid_reachable_from:
        disassembly=Path(str(args.elf)+'.math-audit.S').read_text(encoding='utf-8')
        _,edges=scan(disassembly)
        bad=reachable_forbidden(result['calls'],edges,args.forbid_reachable_from)
        if bad:
            for call in bad:
                print(f"reachable arithmetic: {call['caller']} -> {call['target']}")
            parser.exit(1,'forbidden arithmetic helper reachable from emitted root\n')
