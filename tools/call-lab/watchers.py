"""Sums up a measure.sh run of a shared screen watcher by watcher, over time: what each
watcher received (frames a second, lines, kbit/s), and what the sharer sent it (its
encoder, bitrate, target, the connection's estimate, why WebRTC held it back, losses and
round trips), with the worst 10 s, the time under 5 frames a second and, when the run
had an upload applied part way (LAB_UPLOAD_AFTER), how long each watcher took to come back.

Usage: watchers.py <run folder>... (prints plain text)
"""

import collections
import json
import pathlib
import statistics
import sys

WINDOW = 10  # seconds
SLOW = 5  # frames a second


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def show(value, digits=0):
    return "?" if value is None else f"{value:.{digits}f}"


def lines_of(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def upload_at(run):
    """When the upload was applied, in seconds of copy a's measure (0 for before it began),
    and the line upload.sh wrote; (None, None) for none."""
    log = run / "upload.log"
    mark = run / "meet" / "mark-a-measuring"
    if not log.exists():
        return None, None
    for line in log.read_text().splitlines():
        if " upload of ports " in line:
            at = int(line.split()[0])
            start = int(mark.read_text()) / 1000 if mark.exists() else None
            return (max(0.0, at - start) if start else None), line
    return None, None


def uploaded(run):
    """kbit/s that went up and kB dropped there, and the loopback's kbit/s, per 2 s sample."""
    out = []
    log = run / "upload.log"
    if not log.exists():
        return out
    last = None
    for line in log.read_text().splitlines():
        parts = dict(p.split("=", 1) for p in line.split() if "=" in p)
        if "lo" not in parts:
            continue
        try:
            now = (int(line.split()[0]), int(parts.get("sent") or 0),
                   int(parts.get("dropped") or 0), int(parts["lo"]))
        except ValueError:
            continue
        if last and now[0] > last[0]:
            dt = now[0] - last[0]
            out.append(((now[1] - last[1]) * 8 / dt / 1000, now[2] - last[2],
                        (now[3] - last[3]) * 8 / dt / 1000))
        last = now
    return out


def windows(series, width, of=lambda part: sum(part) / len(part)):
    """[of] (the mean) every run of samples spanning [width] seconds: (start t, value)."""
    out = []
    for i in range(len(series)):
        t0 = series[i][0]
        part = [v for t, v in series[i:] if t < t0 + width]
        if series[-1][0] >= t0 + width - 2.5:
            out.append((t0, of(part)))
    return out


def stretches(series, under):
    """Seconds under [under], in all and the longest stretch, from 2 s samples."""
    total = longest = current = 0.0
    last_t = None
    for t, v in series:
        step = t - last_t if last_t is not None else 2.0
        last_t = t
        if v < under:
            total += step
            current += step
            longest = max(longest, current)
        else:
            current = 0.0
    return total, longest


def summary(run):
    run = pathlib.Path(run)
    print(f"== {run.name}")
    settings = run / "settings.txt"
    if settings.exists():
        print(settings.read_text().strip())
    by_role = {}
    for path in sorted((run / "meet").glob("measure-?.jsonl")):
        lines = lines_of(path)
        if lines:
            by_role[lines[0]["role"]] = lines
    sharer = None
    for role, lines in by_role.items():
        if any(any(p.get("screenEncoder") for p in line["peers"].values()) for line in lines):
            sharer = role
    if sharer is None:
        print("no copy sent a screen")
        return
    me = {role: lines[0].get("me") for role, lines in by_role.items()}
    applied, how = upload_at(run)
    if how:
        print(f"upload applied at {show(applied)} s of the measure: {how.split(' ', 1)[1]}")
    sent = by_role[sharer]
    cpu = median(line.get("cpu") for line in sent)
    print(f"sharer {sharer}: processor median {show(cpu)} % of a core")
    total = [(line["t"], sum(p.get("screenOutKbps") or 0 for p in line["peers"].values()))
             for line in sent]
    before = [v for t, v in total if applied is not None and t < applied]
    after = [v for t, v in total if applied is None or t >= applied]
    print(f"screen sent to all, kbit/s: median {show(median(before))} before the upload,"
          f" {show(median(after))} under it, last 60 s {show(median(v for t, v in total if t > total[-1][0] - 60))}")
    up = uploaded(run)
    if up:
        print(f"through the upload, kbit/s: median {show(median(u for u, _, _ in up if u > 0))},"
              f" dropped {sum(d for _, d, _ in up)} packets; loopback in all median"
              f" {show(median(l for _, _, l in up))} kbit/s")
    for role in sorted(by_role):
        if role == sharer:
            continue
        mine = [line["peers"].get(me[sharer]) or {} for line in by_role[role]]
        times = [line["t"] for line in by_role[role]]
        fps = [(t, r.get("screenFps") or 0.0) for t, r in zip(times, mine)]
        if not any(r.get("screenSends") for r in mine):
            print(f"{role}: never watched")
            continue
        out = [line["peers"].get(me[role]) or {} for line in sent]
        worst = min(windows(fps, WINDOW), key=lambda w: w[1], default=(None, None))
        slow, longest = stretches(fps, SLOW)
        after_fps = [(t, v) for t, v in fps if applied is None or t >= applied]
        recovery = None
        if applied is not None and applied > 0:
            # The first 10 s after it with every reading at 15 frames a second or more.
            for t0, least in windows(after_fps, WINDOW, min):
                if least >= 15 and t0 > applied:
                    recovery = t0 - applied
                    break
        encoders = collections.Counter(r.get("screenEncoder") for r in out if r.get("screenEncoder"))
        limits = collections.Counter(r.get("screenLimit") for r in out if r.get("screenLimit"))
        print(
            f"{role}: fps median {show(median(v for _, v in after_fps), 1)}"
            f" (before {show(median(v for t, v in fps if applied and t < applied), 1)}),"
            f" worst {WINDOW} s {show(worst[1], 1)} at {show(worst[0])} s,"
            f" under {SLOW} fps {show(slow)} s (longest {show(longest)} s)"
            + (f", back to 15 fps {show(recovery)} s after the upload" if applied else "")
            + ("" if recovery is not None or not applied else " (never)")
        )
        print(
            f"   received: lines median {show(median(r.get('screenHeight') for r in mine))},"
            f" kbit/s median {show(median(r.get('screenInKbps') for r in mine))},"
            f" lost {show(sum(r.get('screenLost') or 0 for r in mine))} packets,"
            f" PLI {show(sum(r.get('screenPli') or 0 for r in mine))}, rtt median {show(median(r.get('rtt') for r in mine))} ms"
        )
        print(
            f"   sent: kbit/s median {show(median(r.get('screenOutKbps') for r in out))}"
            f" (target {show(median(r.get('screenOutTarget') for r in out))}),"
            f" estimate median {show(median(r.get('bwe') for r in out))} lowest {show(min((r.get('bwe') for r in out if r.get('bwe') is not None), default=None))},"
            f" fps median {show(median(r.get('screenOutFps') for r in out), 1)},"
            f" lines {dict(collections.Counter(r.get('screenOutHeight') for r in out))},"
            f" lost {show(median(r.get('screenOutLost') for r in out), 1)} %,"
            f" resent kbit/s {show(median(r.get('screenOutResentKbps') for r in out))},"
            f" key frames {show(sum(r.get('screenOutKeys') or 0 for r in out))}"
        )
        print(f"   encoder {dict(encoders)}; held back by {dict(limits)}")
        print("   fps every 10 s: " + " ".join(show(v) for _, v in fps[::5]))
        print("   lines every 10 s: " + " ".join(str(r.get("screenHeight")) for r in mine[::5]))
        print("   estimate every 10 s: " + " ".join(show(r.get("bwe")) for r in out[::5]))
        print("   sent kbit/s every 10 s: " + " ".join(show(r.get("screenOutKbps")) for r in out[::5]))
    print()


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        summary(arg)
