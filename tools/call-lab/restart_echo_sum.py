"""Summary of restart-echo runs (manifest lines "<build> <variant> <run> <time>"), per
build and variant: echo after the change (the changes without words), each second's
level at b above the steady level before (median [min, max] over changes), the changes
with a burst and the longest; and the words right after the change (double talk), against
the run's reference words: dB lost (median sub-word), and the share of sub-words whose
first or last 100 ms lost more than 6 dB. Usage: tools/call-lab/restart_echo_sum.py <manifest>..."""
import json, pathlib, sys
import numpy as np
rows = []
for m in sys.argv[1:]:
    for l in pathlib.Path(m).read_text().split('\n'):
        p = l.split()
        if len(p) >= 3 and p[2] != 'none':
            rows.append((p[0], p[1], pathlib.Path(p[2])))
def mr(v, f='{:.0f}'):
    v = [x for x in v if x == x]
    return (f + ' [' + f + ', ' + f + ']').format(np.median(v), min(v), max(v)) if v else 'n/a'
groups = {}
for b, v, r in rows:
    j = r / 'restart-echo.json'
    if not j.exists():
        continue
    groups.setdefault((v, b), []).append(json.loads(j.read_text()))
for (v, b) in sorted(groups):
    runs = groups[(v, b)]
    ch = [c for a in runs for c in a['changes']]
    echo = [c for c in ch if 'secs' in c]
    dt = [w for c in ch if 'dt' in c for w in c['dt'] if w['corr'] > 0.3]
    print(f"{b} {v}: {len(runs)} runs, {len(echo)} changes for the echo, {len(dt)} words in double talk")
    if echo:
        rel = np.array([[s - c['steady'] for s in c['secs']] for c in echo])
        absl = np.array([c['secs'] for c in echo])
        print("   steady before " + mr([c['steady'] for c in echo]) + " dBFS; a's microphone (echo) " + mr([c['mic_steady'] for c in echo]) + " dBFS")
        print("   each second after, above steady: " + ' | '.join(f"{s}-{s+1} s {mr(rel[:, s])}" for s in range(5)))
        print("   each second after, dBFS: " + ' | '.join(f"{mr(absl[:, s])}" for s in range(5)))
        bursts = [c['bursts'] for c in echo]
        withb = [x for x in bursts if x]
        longest = [max(y[1] for y in x) for x in withb]
        peak = [max(y[2] for y in x) - c['mic_steady'] for x, c in zip(bursts, echo) if x]
        print(f"   bursts: {len(withb)} of {len(echo)} changes; longest " + (mr(longest, '{:.2f}') + ' s' if longest else '-') + "; loudest against the echo at a's microphone " + (mr(peak) + ' dB' if peak else '-'))
    if dt:
        loss = np.array([w['loss'] for w in dt])  # words x 3 sub-words x (whole, first, last)
        print(f"   words after the change: lost {mr(loss[:, :, 0].ravel(), '{:.1f}')} dB; starts >6 dB {100 * np.mean(loss[:, :, 1] > 6):.0f} %, ends >6 dB {100 * np.mean(loss[:, :, 2] > 6):.0f} %, whole >6 dB {100 * np.mean(loss[:, :, 0] > 6):.0f} %")
        for what in ('dt0.5', 'dt3.5'):
            l = np.array([w['loss'] for w in dt if w['what'] == what])
            if len(l):
                print(f"     {what}: lost {mr(l[:, :, 0].ravel(), '{:.1f}')} dB, starts >6 {100 * np.mean(l[:, :, 1] > 6):.0f} %, ends >6 {100 * np.mean(l[:, :, 2] > 6):.0f} %")
