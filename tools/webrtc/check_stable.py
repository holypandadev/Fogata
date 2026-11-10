#!/usr/bin/env python3
"""tools/webrtc/pins.sh against Chrome's Stable channel: is Fogata's WebRTC behind?

    python3 -I tools/webrtc/check_stable.py

Asks chromiumdash (Chrome's release dashboard) which milestone is on the Stable channel for
desktop now, with the WebRTC commit it ships, and which WebRTC branch-head each milestone
has, then compares them with the milestone, branch and commit pins.sh pins. Security fixes
of WebRTC land on the branch-head of the milestone Chrome ships (docs/native-components.md):

- the same milestone, the same commit: up to date;
- the same milestone, another commit: newer merges on the branch-head; a warning;
- one milestone behind: a warning (Chrome moves every four weeks; the move is planned);
- two or more behind: a failure, exit status 1: that branch-head no longer takes fixes.

Prints GitHub's ::warning:: and ::error:: lines, which the weekly workflow shows.
"""

import json
import pathlib
import re
import sys
import urllib.request

sys.dont_write_bytecode = True  # no __pycache__ in the repository
PINS = pathlib.Path(__file__).resolve().parent / "pins.sh"
DASH = "https://chromiumdash.appspot.com"
PLATFORMS = ("Linux", "Windows", "Mac")


def pinned(text):
    """The milestone, branch-head number and commit pins.sh names."""
    version = re.search(r"^LIBWEBRTC_VERSION=fogata-libwebrtc\.m(\d+)\.(\d+)\.", text, re.M)
    branch = re.search(r"^WEBRTC_BRANCH=refs/branch-heads/(\d+)$", text, re.M)
    commit = re.search(r"^WEBRTC_COMMIT=([0-9a-f]{40})$", text, re.M)
    if not (version and branch and commit):
        raise SystemExit("check_stable.py: cannot read LIBWEBRTC_VERSION, WEBRTC_BRANCH and "
                         "WEBRTC_COMMIT in pins.sh")
    if version.group(2) != branch.group(1):
        raise SystemExit("check_stable.py: LIBWEBRTC_VERSION and WEBRTC_BRANCH name different "
                         "branch-heads")
    return int(version.group(1)), branch.group(1), commit.group(1)


def verdict(pins, stable, branches):
    """(level, text) for `pins` (milestone, branch, commit) against `stable`, the newest
    Stable release of one platform as chromiumdash gives it, and `branches`, {milestone:
    WebRTC branch}. Level is "ok", "warning" or "error"."""
    milestone, branch, commit = pins
    shipped = stable["milestone"]
    where = f"Chrome {stable['version']} ({stable['platform']} Stable)"
    if branches.get(milestone) not in (None, branch):
        return "error", (f"pins.sh says M{milestone} is branch-head {branch}, chromiumdash "
                         f"says {branches[milestone]}")
    if milestone >= shipped:
        if stable.get("hashes", {}).get("webrtc") not in (None, commit) and milestone == shipped:
            return "warning", (f"{where} ships WebRTC {stable['hashes']['webrtc'][:12]} of "
                               f"branch-head {branch}; pins.sh has {commit[:12]}: newer merges "
                               "to take")
        return "ok", f"M{milestone} (branch-head {branch}) is what {where} ships, or newer"
    behind = shipped - milestone
    text = (f"pins.sh has M{milestone} (branch-head {branch}); {where} is M{shipped} "
            f"(branch-head {branches.get(shipped, '?')}): {behind} milestone(s) behind")
    return ("warning" if behind == 1 else "error"), text


def fetch(path):
    request = urllib.request.Request(f"{DASH}/{path}",
                                     headers={"User-Agent": "fogata-webrtc-check"})
    with urllib.request.urlopen(request, timeout=60) as answer:
        return json.load(answer)


def main():
    pins = pinned(PINS.read_text())
    branches = {m["milestone"]: m.get("webrtc_branch")
                for m in fetch("fetch_milestones?only_branched=true")}
    worst = "ok"
    for platform in PLATFORMS:
        [stable] = fetch(f"fetch_releases?channel=Stable&platform={platform}&num=1")
        level, text = verdict(pins, stable, branches)
        print(text if level == "ok" else f"::{level}::{text}")
        if level == "error" or (level == "warning" and worst == "ok"):
            worst = level
    sys.exit(1 if worst == "error" else 0)


if __name__ == "__main__":
    main()
