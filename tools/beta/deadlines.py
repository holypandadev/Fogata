#!/usr/bin/env python3
"""The dates after which something of the pre-releases breaks, and how far away each is:

    python3 -I tools/beta/deadlines.py [--online] [--release-json <file>]... [--today <date>]

Each date of DEADLINES below (kept by hand: change it when the thing is renewed), and the
expiry of the newest release (its release.json: `expires`, seconds since 1970), given as a
file with --release-json or, with --online, read from the newest releases on GitHub (the
same page every copy reads). After a release expires no copy takes or installs it; when the
newest one has, a new tester cannot install from a friend or the web at all, until the next
release.

Prints one line per date. Exits 1 when any is WARN_DAYS away or less (or past), so that the
weekly workflow fails and says so; NOTE_DAYS away or less is printed as a warning only. A
release lives 180 days (release.py --days): failing 30 days before its end means a release
at least every 150 days.
"""

import argparse
import datetime
import json
import os
import pathlib
import sys
import urllib.request

GITHUB_REPO = "holypandadev/Fogata"
# Fails the check this many days before a date; only warns this many days before.
WARN_DAYS = 30
NOTE_DAYS = 75

# (date, UTC, what ends then, what to do before). Change a line when its thing is renewed,
# in the same commit as whatever renewed it.
DEADLINES = [
    ("2027-11-12T13:27:00Z",
     "the Apple Development certificate that signs the Mac app expires",
     "make a new one in Xcode, in the same team (the updater pins the team, not the "
     "certificate), and sign the next release with it; installed copies keep running, their "
     "signatures are timestamped (docs/releasing-beta.md, 2d)"),
]


def parse_date(text):
    return datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(
        tzinfo=datetime.timezone.utc)


def release_expiry(manifest):
    """(version, the moment it expires) of a release's manifest (release.json's bytes)."""
    body = json.loads(manifest)
    expires = body.get("expires")
    if not isinstance(expires, int):
        raise ValueError("a release.json without expires")
    when = datetime.datetime.fromtimestamp(expires, tz=datetime.timezone.utc)
    return body.get("version", "?"), when


def fetch(url, token=None):
    request = urllib.request.Request(url, headers={"User-Agent": "fogata-deadlines",
                                                   "Accept": "application/vnd.github+json"})
    if token and url.startswith("https://api.github.com/"):
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request, timeout=30) as answer:
        return answer.read()


def newest_release_json(get=fetch, token=None):
    """The manifest of the newest release on GitHub that has one (pre-releases included, as
    copies take them), or None when none has."""
    releases = json.loads(get(f"https://api.github.com/repos/{GITHUB_REPO}/releases"
                              "?per_page=20", token))
    for release in releases:  # newest first
        if release.get("draft"):
            continue
        for asset in release.get("assets", []):
            if asset.get("name") == "release.json":
                return get(asset["browser_download_url"], token)
    return None


def report(dates, today):
    """Lines saying how far each (moment, what, what to do) is from `today`, and the worst
    level met: "ok", "note" or "warn"."""
    lines, level = [], "ok"
    for when, what, todo in sorted(dates, key=lambda d: d[0]):
        days = (when - today).days
        if days < 0:
            state = f"PAST, {-days} days ago"
        else:
            state = f"in {days} days"
        line = f"{when:%Y-%m-%d}: {what} ({state})"
        if days <= WARN_DAYS:
            lines.append(f"::error::{line}: {todo}")
            level = "warn"
        elif days <= NOTE_DAYS:
            lines.append(f"::warning::{line}: {todo}")
            level = "note" if level == "ok" else level
        else:
            lines.append(line)
    return lines, level


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--online", action="store_true",
                        help="read the newest release's release.json from GitHub")
    parser.add_argument("--release-json", type=pathlib.Path, action="append", default=[])
    parser.add_argument("--today", help="YYYY-MM-DD (by default today, UTC)")
    args = parser.parse_args(argv)
    today = (datetime.datetime.strptime(args.today, "%Y-%m-%d").replace(
        tzinfo=datetime.timezone.utc) if args.today else datetime.datetime.now(
        datetime.timezone.utc))

    dates = [(parse_date(when), what, todo) for when, what, todo in DEADLINES]
    manifests = [path.read_bytes() for path in args.release_json]
    if args.online:
        found = newest_release_json(token=os.environ.get("GH_TOKEN"))
        if found is None:
            print("::warning::no release on GitHub carries a release.json")
        else:
            manifests.append(found)
    for manifest in manifests:
        version, when = release_expiry(manifest)
        dates.append((when, f"the release {version} expires: no copy takes or installs it "
                            "after that",
                      "make a release before (each lives 180 days: one at least every 150)"))
    lines, level = report(dates, today)
    print("\n".join(lines))
    return 1 if level == "warn" else 0


if __name__ == "__main__":
    sys.exit(main())
