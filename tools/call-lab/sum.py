"""Sums up measure.sh runs as Markdown tables: one row a run, and with --copies one row
for each copy of each run.

Usage: sum.py [--copies] <run folder>...
"""

import json
import pathlib
import statistics
import sys


def quantile(values, q):
    values = sorted(v for v in values if v is not None)
    if not values:
        return None
    return values[min(len(values) - 1, round((len(values) - 1) * q))]


def show(value, digits=0):
    if value is None:
        return "?"
    return f"{value:.{digits}f}"


def read(run):
    settings = (run / "settings.txt").read_text().split(",")
    copies = int(settings[0].split()[1])
    mode = settings[1].strip()
    lines = []
    for path in sorted((run / "meet").glob("measure-*.jsonl")):
        lines += [json.loads(line) for line in path.read_text().splitlines() if line]
    load, peaks = [], {}
    proc = run / "proc.log"
    if proc.exists():
        for line in proc.read_text().splitlines():
            parts = line.split()
            load.append(float(parts[2]))
            for part in parts[3:]:
                role, _, kb = part.partition(":")
                if kb:
                    peaks[role] = max(peaks.get(role, 0), int(kb) / 1024)
    return copies, mode, lines, load, peaks


def summary(run):
    copies, mode, lines, load, peaks = read(run)
    rtt, jitter, fps = [], [], []
    lost = received = concealed = samples = 0
    freezes = frozen = stalls = windows = 0
    video_seconds = 0.0
    channel = {"camera": "camera", "screen": "screen"}.get(mode)
    for line in lines:
        for peer in line["peers"].values():
            rtt.append(peer.get("rtt"))
            jitter.append(peer.get("jitter"))
            lost += peer.get("voiceLost") or 0
            received += peer.get("voiceIn") or 0
            concealed += peer.get("concealed") or 0
            samples += peer.get("samples") or 0
            if channel and peer.get(f"{channel}Sends"):
                windows += 1
                video_seconds += line.get("took") or 2
                fps.append(peer.get(f"{channel}Fps"))
                freezes += peer.get(f"{channel}Freezes") or 0
                frozen += peer.get(f"{channel}Frozen") or 0
                stalls += 1 if peer.get(f"{channel}Stall") else 0
    by_copy = {}
    for line in lines:
        by_copy.setdefault(line["role"], []).append(line)
    cpu = [statistics.median([l["cpu"] for l in ls if l["cpu"] is not None]) for ls in by_copy.values()]
    rss = [statistics.median([l["rss"] for l in ls]) for ls in by_copy.values()]
    total = lost + received
    return {
        "copies": copies,
        "mode": mode,
        "seen": len(by_copy),
        "rtt": (quantile(rtt, 0.5), quantile(rtt, 0.9)),
        "loss": 100 * lost / total if total else None,
        "jitter": (quantile(jitter, 0.5), quantile(jitter, 0.9)),
        "concealed": 100 * concealed / samples if samples else None,
        "fps": (quantile(fps, 0.5), quantile(fps, 0.1)),
        "freezes": 60 * freezes / video_seconds if video_seconds else None,
        "frozen": 100 * frozen / video_seconds if video_seconds else None,
        "stalls": (stalls, windows),
        "cpu": (statistics.mean(cpu) if cpu else None, max(cpu) if cpu else None),
        "rss": (statistics.mean(rss) if rss else None, max(peaks.values()) if peaks else None),
        "load": quantile(load, 0.5),
    }


def main(argv):
    per_copy = "--copies" in argv
    # A run still starting has no settings yet.
    runs = [pathlib.Path(a) for a in argv if a != "--copies" and (pathlib.Path(a) / "settings.txt").exists()]
    order = {"idle": 0, "voice": 1, "camera": 2, "screen": 3}
    rows = sorted((summary(r) for r in runs), key=lambda s: (order.get(s["mode"], 9), s["copies"]))
    print(
        "| mode | copies | RTT ms p50/p90 | voice loss % | jitter ms p50/p90 | concealed % "
        "| video fps p50/p10 | freezes /min | frozen % | stalls | CPU % per copy mean/max "
        "| RSS MB mean / peak | load |"
    )
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for s in rows:
        video = s["mode"] in ("camera", "screen")
        print(
            f"| {s['mode']} | {s['copies']}{'' if s['seen'] == s['copies'] else ' (' + str(s['seen']) + ' measured)'} "
            f"| {show(s['rtt'][0])}/{show(s['rtt'][1])} | {show(s['loss'], 2)} "
            f"| {show(s['jitter'][0], 1)}/{show(s['jitter'][1], 1)} | {show(s['concealed'], 2)} "
            f"| {show(s['fps'][0], 1) + '/' + show(s['fps'][1], 1) if video else '-'} "
            f"| {show(s['freezes'], 2) if video else '-'} | {show(s['frozen'], 1) if video else '-'} "
            f"| {str(s['stalls'][0]) + ' of ' + str(s['stalls'][1]) if video else '-'} "
            f"| {show(s['cpu'][0])}/{show(s['cpu'][1])} | {show(s['rss'][0])} / {show(s['rss'][1])} "
            f"| {show(s['load'], 1)} |"
        )
    if not per_copy:
        return
    print()
    print("| mode | copies | copy | CPU % p50/p90 | RSS MB p50 | voice in pkt/s | voice lost | video fps in p50 |")
    print("|---|---|---|---|---|---|---|---|")
    for run in runs:
        copies, mode, lines, _, _ = read(run)
        channel = {"camera": "camera", "screen": "screen"}.get(mode)
        by_copy = {}
        for line in lines:
            by_copy.setdefault(line["role"], []).append(line)
        for role, ls in sorted(by_copy.items()):
            cpu = [l["cpu"] for l in ls]
            voice_in = sum(p.get("voiceIn") or 0 for l in ls for p in l["peers"].values())
            seconds = sum(l.get("took") or 2 for l in ls)
            lost = sum(p.get("voiceLost") or 0 for l in ls for p in l["peers"].values())
            fps = [p.get(f"{channel}Fps") for l in ls for p in l["peers"].values() if channel and p.get(f"{channel}Sends")]
            print(
                f"| {mode} | {copies} | {role} | {show(quantile(cpu, 0.5))}/{show(quantile(cpu, 0.9))} "
                f"| {show(quantile([l['rss'] for l in ls], 0.5))} | {show(voice_in / seconds if seconds else None)} "
                f"| {int(lost)} | {show(quantile(fps, 0.5), 1) if fps else '-'} |"
            )


if __name__ == "__main__":
    main(sys.argv[1:])
