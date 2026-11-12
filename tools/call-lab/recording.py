"""Prints what every process records from the sound server now, one "<pid>:<source name>"
for each recording, sorted; a recording not linked to any source yet shows as
"<pid>:none" (for an instant when a recording starts or its source is made again: see
mic_check.py, which takes only one that stays so as a fault). Usage: recording.py"""

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


def sources():
    return {s.get("index"): s.get("name") for s in pactl("list", "short", "sources")}


# The sources are asked for before and after the recordings: one made between two
# questions would leave its recording without a name.
names = sources()
outputs = pactl("list", "source-outputs")
names = {**names, **sources()}
now = []
for output in outputs:
    pid = output.get("properties", {}).get("application.process.id", "?")
    now.append(f"{pid}:{names.get(output.get('source'), 'none')}")
print(" ".join(sorted(now)))
