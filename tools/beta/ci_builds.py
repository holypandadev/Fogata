#!/usr/bin/env python3
"""The release builds made on GitHub's computers (.github/workflows/release-build.yml), for
tools/beta/release.py, which signs them here: no key ever goes to GitHub.

    python3 -I tools/beta/ci_builds.py fetch <run id> --out <folder> [--only linux,windows,macos]
        [--commit <sha>] [--macos-team <team>]

Downloads the run's builds (`gh run download`), and keeps each only when GitHub's
attestation of it verifies (`gh attestation verify`: made by release-build.yml of
holypandadev/Fogata, from the commit checked out here, on GitHub's own computers) and its
SHA-256 is the one the run listed. The run must have passed, for that commit. Then it lays
them out for release.py: the Linux bundle unpacked (--linux-bundle), the Windows zip
(--windows-zip), and the macOS app, unsigned, which the Mac signs (tools/sign-macos.sh) and
zips (--macos-zip): the commands to run are printed at the end. With --macos-team, the team
pinned into the macOS updater must be that one.

    python3 -I tools/beta/ci_builds.py check-marker <folder> <commit>

What the workflow runs on each build: every commit marker in the files of <folder>
(fogata_update::COMMIT_MARKER, in the release build of the library) must say <commit>, with
no `+dirty`, or release.py would refuse the build later.
"""

import argparse
import hashlib
import importlib.util
import json
import pathlib
import re
import shutil
import subprocess
import sys
import zipfile

REPO = pathlib.Path(__file__).resolve().parents[2]
GITHUB_REPO = "holypandadev/Fogata"
WORKFLOW = ".github/workflows/release-build.yml"
WORKFLOW_NAME = "Release build"
SUMS_ARTIFACT, SUMS_FILE = "fogata-sha256sums", "SHA256SUMS"
# What the workflow uploads: system -> (artifact, file in it).
BUILDS = {
    "linux": ("fogata-linux-x64", "Fogata-linux-x64-bundle.zip"),
    "windows": ("fogata-windows-x64", "Fogata-windows-x64.zip"),
    "macos": ("fogata-macos-unsigned", "Fogata-macos-unsigned.zip"),
}
MARKER = re.compile(rb"fogata-commit-marker:([0-9a-f]{40}(?:\+dirty)?|unknown);")
TEAM_MARKER = re.compile(rb"fogata-updater-team:([A-Z0-9]*);")


def fail(text):
    sys.exit(f"ci_builds.py: {text}")


def load_release():
    """tools/beta/release.py, for its unpacking of zips."""
    sys.dont_write_bytecode = True  # no __pycache__ in the repository
    spec = importlib.util.spec_from_file_location("beta_release", REPO / "tools/beta/release.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def markers(folder):
    """Every commit marker in the files of `folder` (links left out), as text."""
    found = set()
    for path in sorted(pathlib.Path(folder).rglob("*")):
        if path.is_file() and not path.is_symlink():
            found |= {m.decode() for m in MARKER.findall(path.read_bytes())}
    return found


def check_marker(folder, commit):
    found = markers(folder)
    if found != {commit}:
        fail(f"{folder} was built from {', '.join(sorted(found)) or 'no known commit'}, not "
             f"from {commit} with no changes: a file the build reads changed in the checkout "
             "(git status says which)")
    print(f"{folder}: built from {commit}")


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def gh(*args):
    """Runs gh; its output, or a stop with what it said."""
    print("$ gh", " ".join(args))
    result = subprocess.run(["gh", *args], capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"gh {' '.join(args[:2])} failed: {(result.stderr or result.stdout).strip()}")
    return result.stdout


def checked_out_commit():
    result = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--verify", "HEAD^{commit}"],
                            capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"git rev-parse: {result.stderr.strip()}")
    return result.stdout.strip()


def check_run(run, commit, gh=gh):
    """Stops unless `run` is a run of the release workflow that passed, for `commit`."""
    view = json.loads(gh("run", "view", str(run), "--repo", GITHUB_REPO, "--json",
                         "workflowName,headSha,status,conclusion"))
    if view.get("workflowName") != WORKFLOW_NAME:
        fail(f"run {run} is of the workflow {view.get('workflowName')!r}, not {WORKFLOW_NAME!r}")
    if view.get("headSha") != commit:
        fail(f"run {run} built {view.get('headSha')}, not {commit}, the commit checked out")
    if (view.get("status"), view.get("conclusion")) != ("completed", "success"):
        fail(f"run {run} is {view.get('status')} / {view.get('conclusion')}, not a run that "
             "passed")


def check_attestation(path, digest, commit, gh=gh):
    """Stops unless GitHub holds an attestation of `path` made by the release workflow of
    this repository from `commit`, on GitHub's own computers, for this very file."""
    found = json.loads(gh("attestation", "verify", str(path), "--repo", GITHUB_REPO,
                          "--signer-workflow", f"{GITHUB_REPO}/{WORKFLOW}",
                          "--source-digest", commit, "--deny-self-hosted-runners",
                          "--format", "json"))
    subjects = set()
    for result in found if isinstance(found, list) else []:
        statement = result.get("verificationResult", {}).get("statement", {})
        subjects |= {s.get("digest", {}).get("sha256") for s in statement.get("subject", [])}
    if digest not in subjects:
        fail(f"no attestation that verifies names {path.name} with SHA-256 {digest}")


def listed_sums(path):
    sums = {}
    for line in path.read_text().splitlines():
        parts = line.split()
        if len(parts) == 2 and re.fullmatch(r"[0-9a-f]{64}", parts[0]):
            sums[parts[1].lstrip("*")] = parts[0]
    return sums


def updater_team(app):
    """The team pinned into the macOS updater of `app` (tools/beta/macos-updater.sh --team),
    "" when none was."""
    helper = app / "Contents/Library/Helpers/Fogata Updater.app/Contents/MacOS/Fogata Updater"
    if not helper.is_file():
        fail("the macOS app has no updater helper")
    teams = {m.decode() for m in TEAM_MARKER.findall(helper.read_bytes())}
    if len(teams) != 1:
        fail(f"cannot read the team pinned into the updater: {sorted(teams)}")
    return teams.pop()


def fetch(run, out, commit, only, macos_team=None, gh=gh):
    """Downloads, verifies and lays out the builds of `only` from `run`; returns the paths
    for release.py, by system."""
    if out.exists():
        fail(f"{out} is there already")
    check_run(run, commit, gh)
    download = out / "download"
    download.mkdir(parents=True)
    names = [SUMS_ARTIFACT] + [BUILDS[system][0] for system in only]
    gh("run", "download", str(run), "--repo", GITHUB_REPO, "--dir", str(download),
       *[arg for name in names for arg in ("--name", name)])
    sums_file = download / SUMS_ARTIFACT / SUMS_FILE
    if not sums_file.is_file():
        fail(f"the run has no {SUMS_FILE}")
    sums = listed_sums(sums_file)
    release = load_release()
    laid = {}
    for system in only:
        artifact, name = BUILDS[system]
        path = download / artifact / name
        if not path.is_file():
            fail(f"the run has no {artifact}/{name}")
        digest = sha256(path)
        if sums.get(name) != digest:
            fail(f"{name} has SHA-256 {digest}; the run listed {sums.get(name) or 'none'}")
        check_attestation(path, digest, commit, gh)
        print(f"{name}: SHA-256 {digest}, attested")
        if system == "linux":
            release.unzip(path, out / "linux")
            bundle = out / "linux/bundle"
            if not (bundle / "fogata").is_file():
                fail(f"{name} holds no bundle/fogata")
            laid[system] = bundle
        elif system == "macos":
            release.unzip(path, out / "macos")
            app = out / "macos/Fogata.app"
            if not (app / "Contents/Info.plist").is_file():
                fail(f"{name} holds no Fogata.app at its top")
            team = updater_team(app)
            if macos_team is not None and team != macos_team:
                fail(f"the macOS updater was built for team {team or 'none'}, not {macos_team}")
            laid[system] = app
        else:
            laid[system] = out / name
            shutil.copy(path, laid[system])
    with open(out / SUMS_FILE, "w") as file:
        for system in only:
            name = BUILDS[system][1]
            file.write(f"{sums[name]}  {name}\n")
    shutil.rmtree(download)
    return laid


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    get = commands.add_parser("fetch")
    get.add_argument("run", type=int)
    get.add_argument("--out", type=pathlib.Path, required=True)
    get.add_argument("--only", default="linux,windows,macos")
    get.add_argument("--commit", help="by default the commit checked out here")
    get.add_argument("--macos-team")
    marker = commands.add_parser("check-marker")
    marker.add_argument("folder", type=pathlib.Path)
    marker.add_argument("commit")
    args = parser.parse_args()

    if args.command == "check-marker":
        check_marker(args.folder, args.commit)
        return
    only = [system for system in args.only.split(",") if system]
    unknown = [system for system in only if system not in BUILDS]
    if unknown or not only:
        fail(f"--only takes {', '.join(BUILDS)}, not {args.only}")
    commit = args.commit or checked_out_commit()
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        fail(f"{commit} is not a full commit")
    out = args.out.resolve()
    laid = fetch(args.run, out, commit, only, args.macos_team)
    print(f"\nVerified builds of {commit} in {out} (SHA-256 in {out / SUMS_FILE}).")
    if "macos" in laid:
        print("On the Mac, at that commit, sign the app and zip it (docs/releasing-beta.md):\n"
              f'  tools/sign-macos.sh "<identity>" {laid["macos"]}\n'
              f"  ditto -c -k --keepParent {laid['macos']} Fogata-macos-arm64.zip")
    given = {"linux": "--linux-bundle", "windows": "--windows-zip"}
    paths = " ".join(f"{given[s]} {laid[s]}" for s in ("linux", "windows") if s in laid)
    if "macos" in laid:
        paths += " --macos-zip Fogata-macos-arm64.zip"
    print(f"Then: python3 -I tools/beta/release.py --build <build> --version <version> "
          f"--min-build <build> --notes notes.txt {paths} --out <folder>")


if __name__ == "__main__":
    main()
