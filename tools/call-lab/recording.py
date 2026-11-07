"""Prints what every process records from the sound server now, one "<pid>:<source name>"
for each recording, sorted; a recording not linked to any source yet shows as
"<pid>:none". Usage: recording.py"""

import json
import subprocess


def pactl(*args):
    out = subprocess.run(
        ["pactl", "-f", "json", *args], capture_output=True, text=True, check=False
    ).stdout
    try:
        return json.loads(out or "[]")
    except json.JSONDecodeError:
        return []


names = {s.get("index"): s.get("name") for s in pactl("list", "short", "sources")}
now = []
for output in pactl("list", "source-outputs"):
    pid = output.get("properties", {}).get("application.process.id", "?")
    now.append(f"{pid}:{names.get(output.get('source'), 'none')}")
print(" ".join(sorted(now)))
