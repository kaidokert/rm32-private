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
    """Instruction cost, with wait states charged only where flash is read.

    E215 SS12 / E216 SS5: the first version added `ws` to EVERY load and store.
    On a G071 only a flash read pays `FLASH_ACR.LATENCY` -- SRAM is zero-wait
    and the peripherals sit behind APB at the core clock -- so charging two
    wait states to a stack spill or a TIM16 store has no physical basis. It
    inflated the 507-cycle window by ~126 cycles (2 us), and it made the
    docstring's claim to use `isr_cycles.py`'s model false: that one returns a
    flat 2 for any load/store and charges wait states only under
    `--fetch-model`, and then only to pc-relative loads and taken branches.

    So: literal-pool (pc-relative) loads pay `ws`; nothing else does. Fetch is
    still not modelled, which is why no figure here is a bound in either
    direction.
    """
    a, mn, ops = rows[i]
    c = cost(mn, ops)
    if mn.lower().startswith('ldr') and '[pc' in ops:
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
    # Any `bin/*.rs`, not just shell-pwm: E215 SS18 found that hardcoding one
    # binary silently moved `sag-capture`'s 16 cycles into "other" and made the
    # rollup look version-stable when it was not.
    ('ISR entry shell', ('bin/',)),
    ('PAC register accessors', ('stm32g0-staging',)),
]


def deref(rows, lit_ix, limit=8):
    """The index that actually READS through a register just loaded from a pool.

    `ldr rN,[pc,#imm]` only puts the peripheral ADDRESS in a register; the
    access is the following `ldr rM,[rN]`. E215 SS3: anchoring on the literal
    load put the window start one instruction before the timestamp and
    over-counted it. Returns None rather than guessing.
    """
    m = re.match(r'(r\d+)\s*,', rows[lit_ix][2])
    if not m:
        return None
    reg = m.group(1)
    for i in range(lit_ix + 1, min(lit_ix + 1 + limit, len(rows))):
        mn, ops = rows[i][1].lower(), rows[i][2]
        if mn.startswith(('ldr', 'str')) and re.search(rf'\[{reg}\b', ops):
            return i
    return None


def census(rows, path, ws, back=(), depth=1):
    """Addressing-mode census: version-independent, unlike a line number.

    Distinguishes the traffic that a packing change can remove (stores and
    loads through a register base -- shared state and peripherals) from the
    traffic it cannot (stack spills, literal-pool loads), and from work that is
    not memory traffic at all.

    E215 SS16 / E216 SS16: the first version summed only single-trip
    instruction costs while dividing by the whole-window total, so every share
    was deflated by a constant ~18% and the residual was labelled "other /
    unattributed" when it was entirely taken-branch penalties plus extra loop
    trips. Both now land in the buckets they belong to, so the census sums to
    the window total exactly.
    """
    out = {}
    # Taken-branch penalties, charged to the branch that pays them.
    for a, b in zip(path, path[1:]):
        if b != a + 1:
            e = out.setdefault('branch', [0, 0])
            e[0] += 2  # 3 cycles taken vs 1 not taken
    ps = set(path)
    for s_, d_ in back:
        if s_ in ps and d_ in ps:
            body = sum(insn_cost(rows, i, ws) for i in range(d_, s_ + 1)) + 3
            e = out.setdefault(f'extra loop trips (depth {depth})', [0, 0])
            e[0] += body * max(depth - 1, 0)
            e[1] += (s_ - d_ + 1) * max(depth - 1, 0)
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
                    help='persistence-filter trips at the rung being modelled: '
                         '4 at a 77 us interval, 3 at 64 us, saturating at 12 '
                         'above ~250 us (src/bemf.rs mapped_filter_level). A '
                         'loop on the path is charged depth-1 extra traversals')
    ap.add_argument('--quote-source', action='store_true',
                    help='print the source text of each line. ONLY valid when '
                         'the ELF was built from the current tree -- an '
                         'archived ELF carries the line numbers of ITS source')
    a = ap.parse_args()

    rows, words = llvm_rows(a.elf, a.root)
    lines = gnu_lines(a.elf)
    lits = resolve(rows, words)
    print(f'{a.elf}  root={a.root}  llvm-objdump; wait states on flash reads only')
    print(f'  literals in .text: {len(words)};  instructions: {len(rows)};'
          f'  pc-relative loads resolved: {len(lits)};'
          f'  addresses with a firmware50 line: '
          f'{sum(1 for x, _, _ in rows if lines.get(x))}')

    cnt = sorted(i for i, w in lits.items() if w == TIM17_CNT)
    t16 = sorted(i for i, w in lits.items() if w == TIM16_BASE)
    if not cnt or not t16:
        raise SystemExit('anchors not found -- refusing to report a prefix')

    src = deref(rows, cnt[0])
    arm = deref(rows, t16[0])
    if src is None or arm is None:
        raise SystemExit('could not find the access through an anchor register')
    spent_read = None
    for i in cnt:
        d = deref(rows, i)
        if d is not None and d < arm:
            spent_read = d
    if spent_read is None:
        raise SystemExit('no TIM17 CNT read before the arm -- refusing')
    print(f'  entry stamp:  ix {src} (0x{rows[src][0]:x}) '
          f'{rows[src][1]} {rows[src][2]}')
    print(f'  `spent` read: ix {spent_read} (0x{rows[spent_read][0]:x}) '
          f'{rows[spent_read][1]} {rows[spent_read][2]}')
    print(f'  the arm:      ix {arm} (0x{rows[arm][0]:x}) '
          f'{rows[arm][1]} {rows[arm][2]}')

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

    for label, dst in (('entry -> the `spent` read', spent_read),
                       ('entry -> the arm', arm)):
        print()
        print(f'=== {label}')
        for ws in (0, a.wait_states):
            total, path = longest(rows, succ, back, order, src, dst, ws)
            calls = [i for i in path if rows[i][1].lower() == 'bl']
            if calls:
                raise SystemExit(
                    f'{len(calls)} bl instruction(s) on the path at {calls}: '
                    'callees are not costed here -- refusing rather than '
                    'under-reporting')
            ps = set(path)
            extra, loops_on = 0, []
            for (s_, d_) in back:
                if s_ in ps and d_ in ps:
                    body = sum(insn_cost(rows, i, ws)
                               for i in range(d_, s_ + 1)) + 3
                    extra += body * max(a.depth - 1, 0)
                    loops_on.append((d_, s_, body))
            total += extra
            print(f'  {ws} WS: {total} cycles = {total / 64:.2f} us, '
                  f'{len(path)} instructions on the path')
            for d_, s_, body in loops_on:
                print(f'    loop ix {d_}..{s_}, {body} cy/trip, '
                      f'{a.depth} trips (+{body * (a.depth - 1)} over one)')
            if not loops_on and back:
                print('    no classified loop lies on this path')
            if ws == 0:
                continue

            cen = census(rows, path, ws, back, a.depth)
            csum = sum(v[0] for v in cen.values())
            if csum != total:
                raise SystemExit(f'census {csum} != window total {total}')
            print(f'  addressing-mode census (sums to {csum}, the total):')
            for k, (cy, n) in sorted(cen.items(), key=lambda kv: -kv[1][0]):
                print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  '
                      f'{n:3d} insn  {k}')

            per = {}
            for i in path:
                e = per.setdefault(lines.get(rows[i][0]) or '(no line info)',
                                   [0, 0])
                e[0] += insn_cost(rows, i, ws)
                e[1] += 1
            print(f'  attribution by source line, top {a.top} '
                  f'(single trip, no branch penalties):')
            for k, (cy, n) in sorted(per.items(),
                                     key=lambda kv: -kv[1][0])[:a.top]:
                t = ''
                f, _, ln = k.rpartition(':')
                if f in quoted and ln.isdigit() and int(ln) <= len(quoted[f]):
                    t = '  ' + quoted[f][int(ln) - 1].strip()[:72]
                print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  {n:3d} insn'
                      f'  {k}{t}')

            unmatched = sorted(k for k in per
                               if k != '(no line info)'
                               and not any(p in k for _g, ps_ in GROUPS
                                           for p in ps_))
            if unmatched:
                raise SystemExit(
                    'source files on the path match no group in GROUPS, so '
                    'their cycles would vanish into "other":'
                    + ''.join('\n  ' + u for u in unmatched))
            print('  rolled up (single trip; branch penalties and extra loop '
                  'trips are in the census, not here):')
            named = 0
            for lbl, pats in GROUPS:
                cy = sum(v[0] for k, v in per.items()
                         if any(p in k for p in pats))
                n = sum(v[1] for k, v in per.items()
                        if any(p in k for p in pats))
                named += cy
                if n:
                    print(f'    {cy:5d} cy {100 * cy / total:5.1f}%  '
                          f'{n:3d} insn  {lbl}')
            noline = per.get('(no line info)', [0, 0])[0]
            if noline:
                print(f'    {noline:5d} cy {100 * noline / total:5.1f}%'
                      f'       no line info')
            resid = total - named - noline
            print(f'    {resid:5d} cy {100 * resid / total:5.1f}%'
                  f'       branch penalties + extra loop trips (see census)')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
