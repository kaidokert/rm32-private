"""Attribute COMP's entry-to-arm cost (`spent`) to named source groups.

E213 promised this before cutting anything, and it took three attempts to get
an honest number. The two failures are worth recording because both produced a
plausible figure from a broken measurement:

  1. The first version looked for objdump's pc-relative comment with a `;`
     separator, found no literal pools, and silently fell back to costing the
     whole 738-instruction function -- reporting 11.53 us, which happened to
     sit right on top of the 11 us being explained. A wrong method that lands
     on the expected answer is the worst kind.
  2. The second version parsed branch targets with `isr_cycles.py`'s `target()`
     but ran GNU objdump, whose targets are bare hex. Zero of 85 branches
     resolved, the CFG had no branch edges, and there was no path from entry to
     arm at all -- which is at least a loud failure.

`isr_cycles.py` itself is sound: its `OBJDUMP_DEFAULT` is **llvm-objdump**,
which prints `0x`-prefixed targets (97 of 98 resolve) and resolves every
pc-relative load in a trailing `@ 0x...` comment. This script uses the same
disassembler and the same cost model, and joins GNU objdump's `-l` inline line
table by address for the source attribution.

Method:
  * `src` = the first read of TIM17 CNT -- `comp_root`'s entry stamp, the zero
    of `spent`;
  * `dst` = the first access to TIM16 -- the disarm at the top of the arm,
    which is where `spent` is read and the arm begins;
  * the reported cost is the LONGEST PATH from `src` to `dst` over the
    back-edge-free DAG, with `isr_cycles.py`'s taken-branch and load/store
    costs. An index range would include the rejected-crossing code the
    accepted path branches over.

The prediction under test (E213 #1): the six atomic stores and the four
comparator reads together are more than half of this path. Under a third and
items 1-3 cannot deliver, and E213 says to report that and stop.
"""
import argparse
import pathlib
import re
import subprocess
import sys
from collections import deque

sys.path.insert(0, 'scripts')
from isr_cycles import COND, OBJDUMP_DEFAULT, cost, target  # noqa: E402

GNU = 'arm-none-eabi-objdump'
TIM17_CNT = 0x40014824   # the microsecond clock: `hw::clock::raw()`
TIM16_BASE = 0x40014400  # the COM timer: the arm's first register touch

LLVM_LINE = re.compile(r'^\s*([0-9a-f]+):\s+([0-9a-f ]+?)\s*\t([^\t]+)\t?(.*)$')
SRC = re.compile(r'^(.*[/\\][^/\\]+\.rs):(\d+)$')


def llvm_rows(elf, root):
    """(addr, mnemonic, operands) for `root`, plus addr -> literal word."""
    out = subprocess.run([OBJDUMP_DEFAULT, '-d', '--section=.text', elf],
                         capture_output=True, text=True).stdout
    rows, words, inside = [], {}, False
    for raw in out.splitlines():
        h = re.match(r'^([0-9a-f]+) <(.+)>:$', raw.strip())
        if h:
            inside = h.group(2) == root
            continue
        m = LLVM_LINE.match(raw)
        if not m:
            continue
        a, mn, ops = int(m.group(1), 16), m.group(3).strip(), m.group(4).strip()
        if mn == '.word':
            words[a] = int(ops.split()[0], 16)
        elif inside:
            rows.append((a, mn, ops))
    return rows, words


def gnu_lines(elf):
    """addr -> the firmware50 source line in force, from GNU's inline table.

    Only firmware50's own lines name a group: core's inlined
    read_volatile/write_volatile belong to the caller's line, and objdump
    reprints the outer line when an inlined chunk ends.
    """
    out = subprocess.run([GNU, '-d', '-l', '--section=.text', elf],
                         capture_output=True, text=True).stdout
    per, cur = {}, None
    for raw in out.splitlines():
        s = SRC.match(raw.strip())
        if s:
            if 'firmware50' in s.group(1):
                f = s.group(1).replace('\\', '/').split('firmware50/')[-1]
                cur = f'{f}:{s.group(2)}'
            continue
        m = re.match(r'^\s*([0-9a-f]+):\t', raw)
        if m:
            per[int(m.group(1), 16)] = cur
    return per


def resolve(rows, words):
    """index -> the word a pc-relative load reads (llvm's `@ 0x...` comment)."""
    out = {}
    for i, (a, mn, ops) in enumerate(rows):
        if mn.lower().startswith('ldr') and '[pc' in ops:
            c = re.search(r'@\s*0x([0-9a-f]+)', ops)
            if c:
                w = words.get(int(c.group(1), 16))
                if w is not None:
                    out[i] = w
    return out


def graph(rows):
    """succ / classified back-edges, as isr_cycles.py builds them."""
    addr_ix = {a: i for i, (a, _, _) in enumerate(rows)}
    n = len(rows)
    succ, back = [[] for _ in range(n)], []
    for i, (a, mn, ops) in enumerate(rows):
        m = mn.lower()
        base = m[:-2] if len(m) > 2 and m[-2:] in COND and m.startswith('b') else m
        is_b = base == 'b' or (m.startswith('b') and m[1:3] in COND and len(m) <= 4)
        if m in ('b', 'b.n', 'b.w'):
            t = target(ops)
            if t in addr_ix:
                succ[i].append(addr_ix[t])
                if t <= a:
                    back.append((i, addr_ix[t]))
            continue
        if is_b and m != 'bl':
            t = target(ops)
            if i + 1 < n:
                succ[i].append(i + 1)
            if t in addr_ix:
                succ[i].append(addr_ix[t])
                if t <= a:
                    back.append((i, addr_ix[t]))
            continue
        if (m == 'pop' and 'pc' in ops) or m == 'bx':
            continue
        if i + 1 < n:
            succ[i].append(i + 1)

    def reaches(s, d):
        seen, stack = {d}, [d]
        while stack:
            u = stack.pop()
            if u == s:
                return True
            for v in succ[u]:
                if v not in seen:
                    seen.add(v)
                    stack.append(v)
        return False

    return succ, [(s, d) for s, d in back if reaches(s, d)]


def topo(n, succ, back):
    indeg = [0] * n
    for i in range(n):
        for j in succ[i]:
            if (i, j) not in back:
                indeg[j] += 1
    q = deque(i for i in range(n) if indeg[i] == 0)
    order = []
    while q:
        i = q.popleft()
        order.append(i)
        for j in succ[i]:
            if (i, j) in back:
                continue
            indeg[j] -= 1
            if indeg[j] == 0:
                q.append(j)
    if len(order) != n:
        raise SystemExit('spent_compose: a cycle survives back-edge removal; '
                         'refusing rather than under-reporting')
    return order


def insn_cost(rows, i, ws):
    a, mn, ops = rows[i]
    c = cost(mn, ops)
    if mn.lower().startswith(('ldr', 'str')):
        c += ws
    return c


def longest(rows, succ, back, order, src, dst, ws):
    """Longest src->dst cost and path, with isr_cycles' taken-branch model."""
    be, NEG = set(back), -(10 ** 9)
    best = [NEG] * len(rows)
    prev = [None] * len(rows)
    best[src] = insn_cost(rows, src, ws)
    for i in order:
        if best[i] == NEG or i == dst:
            continue
        for j in succ[i]:
            if (i, j) in be:
                continue
            taken = 3 if j != i + 1 else 1
            cand = best[i] + taken - 1 + insn_cost(rows, j, ws)
            if cand > best[j]:
                best[j] = cand
                prev[j] = i
    if best[dst] == NEG:
        raise SystemExit(f'no path from index {src} to {dst}')
    path, k = [], dst
    while k is not None:
        path.append(k)
        k = prev[k]
    return best[dst], list(reversed(path))


GROUPS = [
    ('COMP root: entry, filter driver, publish stores, arm', ('src/roots.rs',)),
    ('estimator: blanking, persistence, blend/clamp, bookkeeping',
     ('src/bemf.rs',)),
    ('wait/advance/pair arithmetic', ('src/commutation.rs',)),
    ('rate + EventWatch bookkeeping', ('src/rate.rs',)),
    ('critical sections and atomics (vendored)',
     ('portable-atomic', 'cortex-m/src')),
    ('ISR entry shell', ('bin/shell-pwm.rs',)),
    ('PAC register accessors', ('stm32g0-staging',)),
]


def census(rows, path, ws):
    """Addressing-mode census: version-independent, unlike a line number.

    Distinguishes the traffic that a packing change can remove (stores and
    loads through a register base -- shared state and peripherals) from the
    traffic it cannot (stack spills, literal-pool loads), and from work that is
    not memory traffic at all.
    """
    out = {}
    for i in path:
        _, mn, ops = rows[i]
        m = mn.lower()
        if m.startswith(('ldr', 'str')):
            k = ('stack (sp-relative)' if '[sp' in ops
                 else 'literal pool (pc-relative)' if '[pc' in ops
                 else 'shared state / peripheral (register base)')
            k = f'{"store" if m.startswith("str") else "load"}: {k}'
        elif m.startswith(('push', 'pop')):
            k = 'prologue/epilogue'
        elif m in ('cpsid', 'cpsie', 'mrs', 'msr'):
            k = 'PRIMASK (critical section)'
        elif m.startswith('b') and m not in ('bl', 'bx', 'bic', 'bics'):
            k = 'branch'
        elif m.startswith('mul'):
            k = 'multiply'
        else:
            k = 'ALU / move'
        e = out.setdefault(k, [0, 0])
        e[0] += insn_cost(rows, i, ws)
        e[1] += 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--elf', default='captures/elf/14CE44E7.e187.elf')
    ap.add_argument('--root', default='ADC_COMP')
    ap.add_argument('--wait-states', type=int, default=2)
    ap.add_argument('--top', type=int, default=20)
    ap.add_argument('--depth', type=int, default=4,
                    help='persistence-filter trips at the rung being modelled '
                         '(4 at a 77 us interval); a loop on the path is '
                         'charged depth-1 extra body traversals')
    ap.add_argument('--quote-source', action='store_true',
                    help='print the source text of each line. ONLY valid when '
                         'the ELF was built from the current tree -- an '
                         'archived ELF carries the line numbers of ITS source')
    a = ap.parse_args()

    rows, words = llvm_rows(a.elf, a.root)
    lines = gnu_lines(a.elf)
    lits = resolve(rows, words)
    print(f'{a.elf}  root={a.root}  (disassembler: llvm-objdump, as isr_cycles)')
    print(f'  literals in .text: {len(words)};  instructions: {len(rows)};'
          f'  pc-relative loads resolved: {len(lits)};'
          f'  addresses with a firmware50 line: '
          f'{sum(1 for x, _, _ in rows if lines.get(x))}')

    cnt = sorted(i for i, w in lits.items() if w == TIM17_CNT)
    t16 = sorted(i for i, w in lits.items() if w == TIM16_BASE)
    print(f'  TIM17 CNT (0x{TIM17_CNT:08x}) loaded at {cnt}')
    print(f'  TIM16     (0x{TIM16_BASE:08x}) loaded at {t16}')
    if not cnt or not t16:
        raise SystemExit('anchors not found -- refusing to report a prefix')
    src, dst = cnt[0], t16[0]

    succ, back = graph(rows)
    order = topo(len(rows), succ, back)
    print(f'  loops classified: {len(back)} back-edge(s) {back}')

    quoted = {}
    if a.quote_source:
        for f in {(lines.get(x) or ':').rsplit(':', 1)[0] for x, _, _ in rows}:
            if f.startswith(('src/', 'bin/')):
                try:
                    quoted[f] = pathlib.Path(f).read_text(
                        encoding='utf-8').splitlines()
                except OSError:
                    pass

    for ws in (0, a.wait_states):
        total, path = longest(rows, succ, back, order, src, dst, ws)
        ps = set(path)
        extra, loops_on = 0, []
        for (s_, d_) in back:
            if s_ in ps and d_ in ps:
                body = sum(insn_cost(rows, i, ws) for i in range(d_, s_ + 1)) + 3
                extra += body * max(a.depth - 1, 0)
                loops_on.append((d_, s_, body))
        total += extra
        print(f'\n  entry stamp (ix {src}, 0x{rows[src][0]:x}) -> arm '
              f'(ix {dst}, 0x{rows[dst][0]:x}), longest path, {ws} WS: '
              f'{total} cycles = {total / 64:.2f} us, {len(path)} instructions')
        for d_, s_, body in loops_on:
            print(f'    includes a loop at ix {d_}..{s_} '
                  f'({lines.get(rows[d_][0])}), {body} cy/trip, charged '
                  f'{a.depth} trips (+{body * (a.depth - 1)} cy over one)')
        if not loops_on and back:
            print('    no classified loop lies on this path')
        if ws == 0:
            continue
        print('  addressing-mode census (one trip of any loop):')
        for k, (cy, n) in sorted(census(rows, path, ws).items(),
                                 key=lambda kv: -kv[1][0]):
            print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  {n:3d} insn  {k}')
        per = {}
        for i in path:
            e = per.setdefault(lines.get(rows[i][0]) or '(no line info)', [0, 0])
            e[0] += insn_cost(rows, i, ws)
            e[1] += 1
        print(f'  attribution by source line, top {a.top}:')
        for k, (cy, n) in sorted(per.items(), key=lambda kv: -kv[1][0])[:a.top]:
            t = ''
            f, _, ln = k.rpartition(':')
            if f in quoted and ln.isdigit() and int(ln) <= len(quoted[f]):
                t = '  ' + quoted[f][int(ln) - 1].strip()[:72]
            print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  {n:3d} insn  {k}{t}')
        print('  rolled up:')
        named = 0
        for label, pats in GROUPS:
            cy = sum(v[0] for k, v in per.items() if any(p in k for p in pats))
            n = sum(v[1] for k, v in per.items() if any(p in k for p in pats))
            named += cy
            print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  {n:3d} insn  {label}')
        print(f'    {total - named:5d} cy {100 * (total - named) / total:5.1f}%'
              f'       other / unattributed')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
