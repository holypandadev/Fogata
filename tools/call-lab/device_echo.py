"""One device-echo run (device-echo.sh): for each measured event (events.log, `E:<label>`),
from what b played of a (played-b, in pieces):
- the echo: RMS dBFS in each second 0-1 ... 4-5 after the event, the steady level before
  it (-4 to -0.3 s, the previous piece's when a piece starts after it), the time in the
  first 2 s more than 20 dB over the steady 90th percentile, and bursts (0.2 s or more,
  median of 3 frames of 50 ms, over that percentile + 10 dB and -70 dBFS);
- a's words of the event's cycle labelled dt* (right after the event), each sub-word against
  the median of the run's words labelled ref*: dB lost, first and last 100 ms;
- a's WebRTC log (a.out) in the event's cycle: device restarts seen ("started recording
  again by itself"), echo controllers aligned anew and made anew.
Writes <run>/device-echo.json and prints a line per event. Usage: device_echo.py <run>..."""
import json, pathlib, re, sys, time, wave
import numpy as np
RATE = 48000
SUBS = [(0.00, 0.42), (0.65, 1.07), (1.35, 2.00)]


def db(x):
    return 20 * np.log10(np.maximum(x, 1e-9) / 32768)


def rms(x):
    return float(np.sqrt(np.mean(x * x))) if len(x) else 0.0


def pieces(run, name):
    out = []
    for st in sorted(run.glob(f'{name}.*.start'), key=lambda p: int(p.name.split('.')[1])):
        n = st.name.split('.')[1]
        raw, wav = run / f'{name}.{n}.raw', run / f'{name}.{n}.wav'
        if raw.exists():
            d = raw.read_bytes(); d = d[: len(d) // 2 * 2]
            with wave.open(str(wav), 'wb') as w:
                w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE); w.writeframes(d)
            raw.unlink()
        with wave.open(str(wav)) as w:
            x = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(float)
        t0 = int(st.read_text().split()[0]) / 1000
        out.append((t0, x))
    return out


def seg(ps, a, b):
    """Samples from a to b (s since the epoch) from the piece holding a."""
    for t0, x in reversed(ps):
        if t0 <= a and a < t0 + len(x) / RATE:
            return x[int((a - t0) * RATE): int((b - t0) * RATE)]
    return np.zeros(0)


def frames(x, step=0.05):
    n = int(step * RATE); k = len(x) // n
    return db(np.sqrt(np.mean(x[: k * n].reshape(k, n) ** 2, axis=1))) if k else np.zeros(0)


def env(x, step):
    n = len(x) // step
    return np.sqrt(np.mean(x[: n * step].reshape(n, step) ** 2, axis=1))


def find_word(ps, at, we):
    x = seg(ps, at - 0.6, at + 3.6)
    if len(x) < RATE:
        return None
    S = RATE // 200; L = len(we)
    e = np.log10(env(x, S) + 1.0)
    best = None
    for i in range(0, len(e) - L):
        s = e[i: i + L]
        if s.std() == 0:
            continue
        c = float(np.dot((s - s.mean()) / s.std(), we) / L)
        if best is None or c > best[0]:
            best = (c, i)
    return None if best is None else (at - 0.6 + best[1] / 200, best[0])


def subs(ps, on):
    out = []
    for a, b in SUBS:
        s = seg(ps, on + a, on + b)
        out.append([float(db(rms(s))), float(db(rms(s[: RATE // 10]))), float(db(rms(s[-RATE // 10:])))])
    return out


STAMP = re.compile(r'^webrtc (\d\d):(\d\d):(\d\d)\.(\d\d\d) ')


def logged(line, near):
    m = STAMP.match(line)
    if not m:
        return None
    h, mi, s, ms = (int(x) for x in m.groups())
    day = time.localtime(near)
    midnight = time.mktime((day.tm_year, day.tm_mon, day.tm_mday, 0, 0, 0, 0, 0, -1))
    t = midnight + h * 3600 + mi * 60 + s + ms / 1000
    return t + 86400 if t < near - 43200 else t - 86400 if t > near + 43200 else t


for r in sys.argv[1:]:
    run = pathlib.Path(r)
    ps = pieces(run, 'played-b')
    pieces(run, 'mic-a')
    with wave.open(str(run / 'sound' / 'words.wav')) as w:
        ww = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(float)
    we = env(ww, RATE // 200); we = (we - we.mean()) / we.std()
    ev = []
    for l in (run / 'events.log').read_text().split('\n'):
        p = l.split()
        if len(p) >= 4:
            ev.append(dict(k=int(p[0]), act=p[1], t=int(p[2]) / 1000, end=int(p[3]) / 1000))
    words = []
    if (run / 'words.log').exists():
        for l in (run / 'words.log').read_text().split('\n'):
            p = l.split()
            if len(p) >= 3:
                words.append(dict(k=int(p[0]), what=p[1], at=int(p[2]) / 1000))
    ref = []
    for w in words:
        if w['what'].startswith('ref'):
            f = find_word(ps, w['at'], we)
            if f and f[1] > 0.6:
                ref.append(subs(ps, f[0]))
    refm = np.median(np.array(ref), axis=0) if ref else None
    log = [l for l in (run / 'a.out').read_text(errors='replace').splitlines() if STAMP.match(l)] if (run / 'a.out').exists() else []
    near = ev[0]['t'] if ev else time.time()
    stamped = [(logged(l, near), l) for l in log]
    cycles = {}
    for e in ev:
        cycles.setdefault(e['k'], []).append(e)
    out = []
    ks = sorted(cycles)
    for i, k in enumerate(ks):
        c0 = min(e['t'] for e in cycles[k])
        c1 = min(e['t'] for e in cycles[ks[i + 1]]) if i + 1 < len(ks) else c0 + 60
        acts = [x for x in cycles[k] if not x['act'].startswith('E:')]
        for e in cycles[k]:
            if not e['act'].startswith('E:'):
                continue
            t = e['t']
            # The marks from the action just before the event to the next action (or the
            # cycle's end).
            w0 = max([x['t'] for x in acts if x['t'] <= t] or [c0])
            w1 = min([x['t'] for x in acts if x['t'] > t] or [c1])
            inlog = [l for tt, l in stamped if tt is not None and w0 - 0.05 <= tt < w1]
            marks = dict(restart=sum('started recording again by itself' in l for l in inlog),
                         aligned=sum('echo controller aligned anew' in l for l in inlog),
                         made=sum('echo controller made anew' in l for l in inlog))
            d = dict(k=k, label=e['act'][2:], marks=marks, acts=[x['act'] for x in cycles[k]])
            before = seg(ps, t - 4, t - 0.3)
            fb = frames(before)
            d['steady'] = float(db(rms(before))) if len(before) else None
            p90 = float(np.percentile(fb, 90)) if len(fb) else -90.0
            d['secs'] = [float(db(rms(seg(ps, t + s, t + s + 1)))) for s in range(5)]
            # When the echo stopped coming through in the first 10 s: the end of the
            # last 50 ms louder than the steady 90th percentile + 20 dB (or -70 dBFS).
            f10 = frames(seg(ps, t, t + 10))
            over = np.nonzero(f10 > max(p90 + 20, -70))[0]
            d['leak_until'] = float((over[-1] + 1) * 0.05) if len(over) else 0.0
            f2 = frames(seg(ps, t, t + 2))
            d['over20_ms'] = int(50 * np.sum(f2 > max(p90 + 20, -70)))
            d['peak2'] = float(f2.max()) if len(f2) else None
            f5 = frames(seg(ps, t, t + 5))
            fs = np.array([np.median(f5[max(0, j - 1): j + 2]) for j in range(len(f5))])
            bar = max(p90 + 10, -70)
            runs_, cur, start = [], 0, 0
            for j, o in enumerate(list(fs > bar) + [False]):
                if o:
                    if cur == 0:
                        start = j
                    cur += 1
                else:
                    if cur >= 4:
                        runs_.append((start * 0.05, cur * 0.05, float(f5[start: start + cur].max())))
                    cur = 0
            d['bursts'] = runs_
            dt = []
            for w in words:
                if w['k'] == k and w['what'].startswith('dt') and refm is not None:
                    fw = find_word(ps, w['at'], we)
                    if fw:
                        dt.append(dict(what=w['what'], corr=fw[1], loss=(refm - np.array(subs(ps, fw[0]))).tolist()))
            d['dt'] = dt
            out.append(d)
            print(run.name, f"k{k} {d['label']}: marks {marks}; over20 {d['over20_ms']} ms, bursts {len(runs_)}"
                  + (f" longest {max(b[1] for b in runs_):.2f}s" if runs_ else '')
                  + f"; leak until {d['leak_until']:.2f} s; secs {' '.join(f'{s:.0f}' for s in d['secs'])} (steady {d['steady'] if d['steady'] is None else round(d['steady'])})"
                  + (f"; words lost {' '.join(str(round(float(np.median(np.array(x['loss'])[:, 0])), 1)) for x in dt)}" if dt else ''))
    (run / 'device-echo.json').write_text(json.dumps(dict(events=out, ref_words=len(ref))))
