"""Paired restart-echo runs: each output-change run (far) against its exact control (none: the
same call, loops and words, no output chosen), word by word.
Input: a list file of lines "<build> <far run> <control run>" (restart_echo.py run on both first,
so each holds restart-echo.json). For every word said in double talk (dt*, the odd changes) the
loss is the mean over its three sub-words of the dB lost against the run's reference words; the
pair's figure is far - control for the same change and word: the realign's own cost.
Per build: median and range of that cost (all words, dt0.5, dt3.5), the share over 6 dB, and the
echo bursts of far and of control (even changes).
Usage: tools/call-lab/restart_echo_pair.py <list>"""
import json, pathlib, sys
import numpy as np


def words(run):
    d = json.loads((pathlib.Path(run) / 'restart-echo.json').read_text())
    out, bursts = {}, 0
    for c in d['changes']:
        for w in c.get('dt', []):
            out[(c['k'], w['what'])] = float(np.mean(np.array(w['loss'])[:, 0]))
        bursts += len(c.get('bursts', []))
    return out, bursts


def summary(xs):
    if not xs:
        return 'none'
    a = np.array(xs)
    return f"{np.median(a):.1f} [{a.min():.1f}, {a.max():.1f}] dB, over 6 dB {100 * np.mean(a > 6):.0f} % ({len(a)})"


builds = {}
for line in pathlib.Path(sys.argv[1]).read_text().split('\n'):
    p = line.split()
    if len(p) != 3:
        continue
    b, far, ctl = p
    wf, bf = words(far)
    wc, bc = words(ctl)
    r = builds.setdefault(b, dict(all=[], dt0=[], dt3=[], far_b=0, ctl_b=0, pairs=0))
    r['pairs'] += 1
    r['far_b'] += bf
    r['ctl_b'] += bc
    for key, loss in wf.items():
        if key in wc:
            cost = loss - wc[key]
            r['all'].append(cost)
            (r['dt0'] if key[1].startswith('dt0') else r['dt3']).append(cost)
for b, r in builds.items():
    print(f"{b}: {r['pairs']} pairs; the realign's cost per word {summary(r['all'])}")
    print(f"   dt0.5 {summary(r['dt0'])}; dt3.5 {summary(r['dt3'])}")
    print(f"   echo bursts: far {r['far_b']}, control {r['ctl_b']}")
