"""Fail-closed recognition of the static ADC_COMP persistence loop, not WCET."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def inspect(disassembly):
    instructions={}
    for line in disassembly.splitlines():
        m=re.match(r'\s*([0-9a-f]+):\s+(?:[0-9a-f]{4,8}\s+)+([a-z.][a-z0-9.]*)\s*(.*)',line)
        if m:instructions[int(m[1],16)]=(m[2],m[3])
    start=re.search(r'^([0-9a-f]+) <ADC_COMP>:',disassembly,re.M)
    if not start:raise ValueError('missing ADC_COMP symbol')
    begin=int(start[1],16)
    following=re.search(r'^([0-9a-f]+) <[^>]+>:',disassembly[start.end():],re.M)
    if not following:raise ValueError('missing next symbol boundary')
    end=int(following[1],16)
    candidates=[]
    def target(operand):return int(operand.split()[0],16)
    for edge,(op,args) in instructions.items():
        if not begin<=edge<end or op!='beq.n':continue
        top=target(args)
        if not begin<=top<edge:continue
        addresses=list(range(top,edge+2,2))
        if any(a not in instructions for a in addresses):continue
        sequence=[instructions[a] for a in addresses]
        if [x[0] for x in sequence[:3]]!=['cmp','bcc.n','b.n']:continue
        if target(sequence[1][1])!=top+6 or top<=target(sequence[2][1])<=edge:continue
        path=addresses[:2]+addresses[3:]
        allowed={'cmp','mov','movs','ldr','ands','negs','adcs','subs','sbcs','eors','adds'}
        if any(instructions[a][0] not in allowed for a in path[2:-1]):continue
        loads=[]
        for n,a in enumerate(path[:-1]):
            op,args=instructions[a]
            literal=re.match(r'(r\d+), \[pc, #\d+\].*@ \(([0-9a-f]+) ',args)
            if op!='ldr' or not literal:continue
            word=instructions.get(int(literal[2],16))
            if word is None or word[0]!='.word' or int(word[1],16)!=0x40010204:continue
            next_op,next_args=instructions[path[n+1]]
            if next_op=='ldr' and re.fullmatch(r'r\d+, \['+literal[1]+r', #0\]',next_args):
                loads.append(path[n+1])
        if len(loads)!=1:continue
        candidates.append(dict(top=hex(top),back_edge=hex(edge),comparator_load=hex(loads[0]),
                               successful_iteration_instructions=len(path),
                               instructions=[dict(address=hex(a),opcode=instructions[a][0],operands=instructions[a][1]) for a in path]))
    if len(candidates)!=1:raise ValueError(f'expected one recognized static persistence loop, got {len(candidates)}')
    return dict(**candidates[0],calls_inside_loop=False,cycles_measured=False,
                peripheral_waits_known=False,preemption_excluded=False,
                physical_persistence_duration_proven=False)


def disassemble(path):
    return subprocess.check_output(['arm-none-eabi-objdump','-d','-C',str(path)],text=True)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('elf',nargs='+',type=Path)
    for path in ap.parse_args().elf:
        print(json.dumps(dict(elf=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                              **inspect(disassemble(path))),indent=2))
