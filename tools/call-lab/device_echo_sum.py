"""Sums device-echo runs up (lines "<build> <case> <run>"), per case and build: the
events without words of a's right after them (the echo): time in the first 2 s over the
steady level + 20 dB (or -70 dBFS), loudest 50 ms, events with a burst and the longest,
the first seconds' levels; the words right after the events (dt*): dB lost and the share
of sub-words whose first or last 100 ms lost more than 6 dB; and the marks seen in the
log at each event (device restarts seen / aligned anew / made anew).
Usage: device_echo_sum.py <list>..."""
import collections, json, pathlib, sys
import numpy as np
rows = []
for m in sys.argv[1:]:
    for l in pathlib.Path(m).read_text().split('\n'):
        p = l.split()
        if len(p) >= 3 and p[2] != 'none':
            rows.append((p[0], p[1], pathlib.Path(p[2])))
def mr(v, f='{:.0f}'):
    v = [x for x in v if x is not None and x == x]
    return (f + ' [' + f + ', ' + f + ']').format(np.median(v), min(v), max(v)) if v else 'n/a'
groups = collections.defaultdict(list)
for b, c, r in rows:
    j = r / 'device-echo.json'
    if j.exists():
        groups[(c, b)].append(json.loads(j.read_text()))
for (c, b) in sorted(groups):
    ev = [e for a in groups[(c, b)] for e in a['events']]
    echo = [e for e in ev if not e['dt']]
    dt = [w for e in ev for w in e['dt'] if w['corr'] > 0.3]
    marks = collections.Counter(f"{e['marks']['restart']}/{e['marks']['aligned']}/{e['marks']['made']}" for e in ev)
    print(f"{b} {c}: {len(groups[(c, b)])} runs, {len(echo)} events for the echo, {len(dt)} words; marks restart/aligned/made: " + ', '.join(f'{k} x{v}' for k, v in sorted(marks.items())))
    if echo:
        withb = [e for e in echo if e['bursts']]
        print("   echo: over20 " + mr([e['over20_ms'] for e in echo]) + " ms; loudest " + mr([e['peak2'] for e in echo]) + " dBFS; bursts "
              + f"{len(withb)}/{len(echo)}" + (", longest " + mr([max(x[1] for x in e['bursts']) for e in withb], '{:.2f}') + " s" if withb else '')
              + "; echo until " + mr([e.get('leak_until') for e in echo], '{:.2f}') + " s"
              + "; seconds " + ' | '.join(mr([e['secs'][s] for e in echo]) for s in range(5)) + " dBFS")
    if dt:
        loss = np.array([w['loss'] for w in dt])
        print(f"   words: lost {mr(loss[:, :, 0].ravel(), '{:.1f}')} dB; starts >6 dB {100 * np.mean(loss[:, :, 1] > 6):.0f} %, ends >6 dB {100 * np.mean(loss[:, :, 2] > 6):.0f} %, whole >6 dB {100 * np.mean(loss[:, :, 0] > 6):.0f} %")
