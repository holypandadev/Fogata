"""One restart-echo run: per output change (changes.log, from the moment asked):
- echo (changes without words in their first 6 s, the even ones): what b played of a, RMS
  dBFS in each second 0-1 ... 4-5 after the change, against the steady level of the 3.7 s
  before it (-4 to -0.3 s); bursts: 0.2 s or more (4 frames of 50 ms, median of 3) louder
  than the steady 90th percentile + 10 dB and than -70 dBFS: count, longest, loudest;
  and the echo as a's microphone heard it (mic-a, same seconds) for scale;
- double talk (the odd changes, words at +0.5 and +3.5 s): each word's three sub-words at
  b, found by the words' envelope, against the median of the same sub-word in the run's
  reference words (10.5 and 13.5 s after every change): dB lost (whole), and whether the
  first and the last 100 ms of each lost more than 6 dB.
Writes <run>/restart-echo.json. Usage: tools/call-lab/restart_echo.py <run>..."""
import json, pathlib, sys, wave
import numpy as np
RATE = 48000
SUBS = [(0.00, 0.42), (0.65, 1.07), (1.35, 2.00)]


def load(run, name):
    raw, wav = run / f'{name}.raw', run / f'{name}.wav'
    if raw.exists():
        d = raw.read_bytes(); d = d[: len(d) // 2 * 2]
        with wave.open(str(wav), 'wb') as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE); w.writeframes(d)
        raw.unlink()
    with wave.open(str(wav)) as w:
        x = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(float)
    return x, int((run / f'{name}.start').read_text().split()[0]) / 1000


def db(x):
    return 20 * np.log10(np.maximum(x, 1e-9) / 32768)


def rms(x):
    return float(np.sqrt(np.mean(x * x))) if len(x) else 0.0


def frames(x, t0, a, b, step=0.05):
    i, j = int((a - t0) * RATE), int((b - t0) * RATE)
    seg = x[max(0, i): max(0, j)]
    n = int(step * RATE); k = len(seg) // n
    return db(np.sqrt(np.mean(seg[: k * n].reshape(k, n) ** 2, axis=1))) if k else np.zeros(0)


def env(x, step):
    n = len(x) // step
    return np.sqrt(np.mean(x[: n * step].reshape(n, step) ** 2, axis=1))


def find_word(x, t0, at, we):
    """Start (s since the epoch) of the word nearest `at` (searched -0.6..+1.4 s)."""
    S = RATE // 200; L = len(we)
    base = int((at - t0) * 200)
    e = np.log10(env(x, S) + 1.0)
    best = None
    for lag in range(-120, 281):
        i = base + lag
        s = e[i: i + L] if i >= 0 else np.zeros(0)
        if len(s) < L or s.std() == 0:
            continue
        c = float(np.dot((s - s.mean()) / s.std(), we) / L)
        if best is None or c > best[0]:
            best = (c, i)
    return None if best is None else (t0 + best[1] / 200, best[0])


def subs(x, t0, on):
    out = []
    for a, b in SUBS:
        i = int((on + a - t0) * RATE); j = int((on + b - t0) * RATE)
        seg = x[i:j]
        out.append([float(db(rms(seg))), float(db(rms(seg[: RATE // 10]))), float(db(rms(seg[-RATE // 10:])))])
    return out


for r in sys.argv[1:]:
    run = pathlib.Path(r)
    pb, tb = load(run, 'played-b'); ma, tm = load(run, 'mic-a')
    for n in ('out1', 'out2'):
        if (run / f'{n}.raw').exists():
            load(run, n)
    with wave.open(str(run / 'sound' / 'words.wav')) as w:
        ww = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(float)
    we = env(ww, RATE // 200); we = (we - we.mean()) / we.std()
    changes = []
    for l in (run / 'changes.log').read_text().split('\n'):
        p = l.split()
        if len(p) >= 6:
            changes.append(dict(k=int(p[0]), out=p[1], asked=int(p[3]) / 1000,
                                done=None if p[5] == 'none' else int(p[5]) / 1000))
    changes.sort(key=lambda c: c['k'])
    words = []
    for l in (run / 'words.log').read_text().split('\n'):
        p = l.split()
        if len(p) >= 4:
            words.append(dict(k=int(p[0]), what=p[1], at=int(p[3]) / 1000))
    # Reference sub-words: every ref word of the run.
    ref = []
    for w in words:
        if w['what'].startswith('ref'):
            f = find_word(pb, tb, w['at'], we)
            if f and f[1] > 0.6:
                ref.append(subs(pb, tb, f[0]))
    refm = np.median(np.array(ref), axis=0) if ref else None
    out = dict(changes=[], ref_words=len(ref))
    for c in changes:
        t = c['asked']
        d = dict(k=c['k'], out=c['out'], took=None if c['done'] is None else c['done'] - t)
        steady = frames(pb, tb, t - 4, t - 0.3)
        d['steady'] = float(db(rms(pb[int((t - 4 - tb) * RATE): int((t - 0.3 - tb) * RATE)])))
        d['steady_p90'] = float(np.percentile(steady, 90)) if len(steady) else None
        d['mic_steady'] = float(db(rms(ma[int((t - 4 - tm) * RATE): int((t - 0.3 - tm) * RATE)])))
        if c['k'] % 2 == 0:
            d['secs'] = [float(db(rms(pb[int((t + s - tb) * RATE): int((t + s + 1 - tb) * RATE)]))) for s in range(5)]
            d['mic_secs'] = [float(db(rms(ma[int((t + s - tm) * RATE): int((t + s + 1 - tm) * RATE)]))) for s in range(5)]
            f = frames(pb, tb, t, t + 5)
            fs = np.array([np.median(f[max(0, i - 1): i + 2]) for i in range(len(f))])
            bar = max(d['steady_p90'] + 10, -70)
            over = fs > bar
            runs_, cur, start = [], 0, 0
            for i, o in enumerate(list(over) + [False]):
                if o:
                    if cur == 0:
                        start = i
                    cur += 1
                else:
                    if cur >= 4:
                        runs_.append((start * 0.05, cur * 0.05, float(f[start: start + cur].max())))
                    cur = 0
            d['bursts'] = runs_
        else:
            dt = []
            for w in words:
                if w['k'] == c['k'] and w['what'].startswith('dt'):
                    fw = find_word(pb, tb, w['at'], we)
                    if fw is None or refm is None:
                        continue
                    sw = np.array(subs(pb, tb, fw[0]))
                    loss = refm - sw
                    dt.append(dict(what=w['what'], corr=fw[1], loss=loss.tolist()))
            d['dt'] = dt
        out['changes'].append(d)
    (run / "restart-echo.json").write_text(json.dumps(out))
    e = [c for c in out['changes'] if 'secs' in c]
    print(run.name, f"ref words {len(ref)};", ' | '.join(
        f"k{c['k']} {c['out'][:9]} steady {c['steady']:.0f} secs {' '.join(f'{s:.0f}' for s in c['secs'])} bursts {len(c['bursts'])}"
        + (f" longest {max(b[1] for b in c['bursts']):.2f}s" if c['bursts'] else '') for c in e))
    for c in out['changes']:
        if 'dt' in c:
            print('  ', f"k{c['k']}", ' '.join(f"{x['what']} corr {x['corr']:.2f} loss {np.round(np.array(x['loss'])[:,0],1).tolist()}" for x in c['dt']))
