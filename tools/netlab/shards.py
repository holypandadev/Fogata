#!/usr/bin/env python3
"""The jobs of .github/workflows/netlab.yml: the lab's scenarios split among a few computers,
and dht-liars run again and again on others, all at once.

    python3 -I tools/netlab/shards.py <shards> <liars> [scenario...]

Every scenario starts its own namespaces and its own folder, so none needs another's state
or another's computer: they are split by how long each took on GitHub's computers (the
longest first, each to the computer with the least so far), and each computer runs its own
in the lab's order. A scenario MINUTES does not know counts as DEFAULT minutes. The list is
the lab's own (`netlab.sh --list`), and this fails unless every scenario of it is in exactly
one shard. Given scenarios run as given, on one computer. dht-liars, <liars> times, is split
between LIARS_JOBS computers. Prints the jobs as JSON for the workflow's matrix, and writes
them to $GITHUB_OUTPUT as `jobs` when there is one.
"""

import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

# Minutes each took on one of GitHub's computers (ubuntu-24.04, November 2026).
MINUTES = {
    "friends-remove": 9.2, "friends-ex-mutual": 4.0, "friends-later": 4.0,
    "friends-later-code": 4.0, "member-lost-direct": 3.6, "direct-apart": 3.5, "dht-liars": 3.1,
    "silent-flood": 2.9, "room-files": 2.5, "group-join": 2.4, "mdns-flood": 2.1,
    "friends-code": 2.0, "friends-link": 2.0, "update-chain": 1.9, "cone-cone-member": 1.8,
    "lan-sealed": 1.8, "tcp-known": 1.8, "lan-first-contact": 1.7, "lan-first-contact-quiet": 1.7,
    "transfer-cone-cone": 1.7, "transfer-two-routers": 1.7, "cone-cone": 1.6,
    "cone-symmetric": 1.6, "cone-symmetric-member": 1.6, "home-two-routers": 1.6,
    "home-two-routers-closed": 1.6, "home-two-routers-firewall": 1.6,
    "symmetric-symmetric-member": 1.6, "tcp-slow": 1.6, "transfer-cone-symmetric": 1.6,
    "transfer-two-routers-back": 1.6, "fake-observed": 1.5, "transfer-lan": 1.4,
    "inbound-flood": 1.2, "mdns-stale": 1.1, "media-silent": 1.1, "update-refusals": 0.8,
    "network-check": 0.7, "network-check-negative": 0.7, "selftest": 0.1,
}
DEFAULT = 1.5
LIARS_JOBS = 2


def lab_scenarios():
    out = subprocess.run(["bash", str(ROOT / "tools/netlab/netlab.sh"), "--list"],
                         check=True, capture_output=True, text=True).stdout
    return out.split()


def split(scenarios, count):
    """`count` lists of scenarios, each in the lab's order, of about the same minutes."""
    loads = [0.0] * count
    shards = [[] for _ in range(count)]
    for name in sorted(scenarios, key=lambda name: (-MINUTES.get(name, DEFAULT), name)):
        least = min(range(count), key=lambda i: (loads[i], i))
        loads[least] += MINUTES.get(name, DEFAULT)
        shards[least].append(name)
    order = {name: at for at, name in enumerate(scenarios)}
    return [sorted(shard, key=order.get) for shard in shards], loads


def jobs(count, liars, given):
    found = []
    if given:
        found.append({"id": "scenarios-1", "name": "scenarios", "scenarios": " ".join(given),
                      "first": 0, "last": 0})
    else:
        scenarios = lab_scenarios()
        if len(set(scenarios)) != len(scenarios):
            sys.exit("netlab.sh --list names a scenario twice")
        for name in sorted(set(MINUTES) - set(scenarios)):
            print(f"::warning::{name} has a time in tools/netlab/shards.py but the lab "
                  "has no such scenario", file=sys.stderr)
        shards, loads = split(scenarios, count)
        placed = [name for shard in shards for name in shard]
        if sorted(placed) != sorted(scenarios) or len(placed) != len(set(placed)):
            sys.exit("the shards do not hold every scenario of the lab exactly once")
        for at, (shard, load) in enumerate(zip(shards, loads), 1):
            if not shard:
                continue
            print(f"scenarios {at} of {count}, about {load:.0f} minutes: {' '.join(shard)}",
                  file=sys.stderr)
            found.append({"id": f"scenarios-{at}", "name": f"scenarios {at} of {count}",
                          "scenarios": " ".join(shard), "first": 0, "last": 0})
    parts = min(LIARS_JOBS, liars)
    first = 1
    for part in range(parts):
        times = liars // parts + (1 if part < liars % parts else 0)
        found.append({"id": f"liars-{part + 1}", "name": f"dht-liars {first}-{first + times - 1}",
                      "scenarios": "", "first": first, "last": first + times - 1})
        first += times
    return found


def main(argv):
    if len(argv) < 2 or not argv[0].isdigit() or not argv[1].isdigit() or int(argv[0]) < 1:
        sys.exit("usage: shards.py <shards> <liars> [scenario...]  (two whole numbers)")
    found = jobs(int(argv[0]), int(argv[1]), argv[2:])
    text = json.dumps(found)
    print(text)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as out:
            out.write(f"jobs={text}\n")


if __name__ == "__main__":
    main(sys.argv[1:])
