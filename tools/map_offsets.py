"""Map main-executable offsets from 13.0.5 to 13.0.4 using a Ghidra project holding both builds.

Code offsets are matched by an instruction window with address immediates masked out
(ADRP/ADR, B/BL, ADD/LDR/STR imm12, LDR literal), searched near the same offset in 13.0.4.
Data offsets are mapped through the code that references them.

Requires PyGhidra (pip install the wheels shipped in <ghidra>/Ghidra/Features/PyGhidra/pypkg/dist).

    python tools/map_offsets.py --ghidra C:/ghidra_12.0.4_PUBLIC --project "D:/Ghidra/Smash Research"         0x33b01b0 0x5187f08

Prints one line per offset: the 13.0.4 offset and how many candidate matches were found.
Anything other than exactly one match should be checked by hand.
"""
import argparse, os, struct

ap = argparse.ArgumentParser()
ap.add_argument("--ghidra", required=True, help="Ghidra install directory")
ap.add_argument("--project", required=True, help="path to the .gpr project, without extension")
ap.add_argument("--new", default="/main_1305", help="program path of the newer build")
ap.add_argument("--old", default="/main_1304", help="program path of the older build")
ap.add_argument("offsets", nargs="+", help="offsets into the newer build")
args = ap.parse_args()

import pyghidra
pyghidra.start(install_dir=args.ghidra)
from ghidra.base.project import GhidraProject
from ghidra.framework.client import HeadlessClientAuthenticator
from ghidra.util.task import TaskMonitor
import jpype

HeadlessClientAuthenticator.installHeadlessClientAuthenticator(None, None, False)
proj = GhidraProject.openProject(os.path.dirname(args.project), os.path.basename(args.project), True)
consumer = jpype.JObject(proj)
def open_prog(path):
    return proj.getProjectData().getFile(path).getReadOnlyDomainObject(consumer, -1, TaskMonitor.DUMMY)
P5 = open_prog(args.new); P4 = open_prog(args.old)
B5 = P5.getImageBase(); B4 = P4.getImageBase()

def read(prog, base, off, n):
    buf = jpype.JArray(jpype.JByte)(n)
    got = prog.getMemory().getBytes(base.add(off), buf)
    return bytes(b & 0xff for b in buf[:got])

def mask(w):
    if (w & 0x1F000000) == 0x10000000: return w & 0x9F00001F          # ADR/ADRP
    if (w & 0x7C000000) == 0x14000000: return w & 0xFC000000          # B/BL
    if (w & 0x7F800000) == 0x11000000: return w & ~(0xFFF << 10)      # ADD/SUB imm (32/64)
    if (w & 0x3B000000) == 0x39000000: return w & ~(0xFFF << 10)      # LDR/STR unsigned imm
    if (w & 0x3B000000) == 0x18000000: return w & 0xFF00001F          # LDR literal
    return w

def masked(bs):
    ws = struct.unpack("<%dI" % (len(bs)//4), bs[:len(bs)//4*4])
    return struct.pack("<%dI" % len(ws), *[mask(w) for w in ws])

def is_exec(prog, base, off):
    blk = prog.getMemory().getBlock(base.add(off))
    return blk is not None and blk.isExecute()

RADIUS = 0x80000
def map_code(off):
    """Return (offset_1304, nmatches, window_words) for a code offset in 1305."""
    lo = max(0, off - RADIUS)
    region = masked(read(P4, B4, lo, 2*RADIUS))
    for before, after in ((12, 12), (6, 10), (0, 8), (8, 0), (4, 4)):
        win = masked(read(P5, B5, off - before*4, (before+after+1)*4))
        hits = []; i = region.find(win)
        while i != -1:
            if i % 4 == 0: hits.append(lo + i + before*4)
            i = region.find(win, i+1)
        if hits:
            best = min(hits, key=lambda h: abs(h - off))
            return best, len(hits), before+after+1
    return None, 0, 0

def map_data(off):
    rm5 = P5.getReferenceManager()
    target5 = B5.add(off)
    froms = [r.getFromAddress() for r in rm5.getReferencesTo(target5)]
    results = {}
    froms = [f for f in froms if f.isMemoryAddress()]
    for f in froms[:6]:
        foff = f.subtract(B5)
        if not is_exec(P5, B5, foff): continue
        m4, n, _ = map_code(foff)
        if m4 is None or n != 1: continue
        for r in P4.getReferenceManager().getReferencesFrom(B4.add(m4)):
            if not r.getToAddress().isMemoryAddress(): continue
            t = r.getToAddress().subtract(B4)
            results[t] = results.get(t, 0) + 1
    if not results: return None, 0
    t = max(results, key=results.get)
    return t, len(results)

for text in args.offsets:
    off = int(text, 0)
    if is_exec(P5, B5, off):
        m, n, _ = map_code(off)
    else:
        m, n = map_data(off)
    print(f"{off:#x} -> {m:#x}" if m is not None else f"{off:#x} -> ?", f"({n} match{'es' if n != 1 else ''})")
P5.release(consumer); P4.release(consumer); proj.close()
