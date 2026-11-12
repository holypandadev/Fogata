# Releasing a pre-release

Pre-releases are the `beta` channel. Testers get their first install from the files of a
pre-release on the project's release page; after that, new versions reach them over Fogata
itself, from friend to friend, signed by the release key (architecture.md, 5f).

There is no paid code signing: the Windows builds are not Authenticode-signed, and the macOS
builds are signed with a free Apple Development certificate but not notarized. So Windows
and macOS warn testers once, before Fogata first opens, and what tells a tester the download
is genuine is its SHA-256 checksum, which you send apart from the download (below).

## The keys (once)

Fogata trusts two keys of yours, both compiled into every build (`RELEASE_KEYS` in
`crates/fogata-update/src/lib.rs`):

- the **release key**, which signs every release and lives on your computer;
- the **rescue key**, which signs only the release that replaces a release key that leaked
  or was lost, and lives on a USB stick kept offline. A release it signs may go further
  ahead of the copies' builds than any release of a release key may (2000 builds against
  1000), so it passes whatever a leaked release key made copies hold.

A build trusts either both or none (a test checks it). Make them on Linux or macOS: the
passphrase is asked on the terminal and not shown; on Windows the command refuses to ask.

1. Build the command-line program, in the repository:

   ```sh
   cargo build --release -p fogata
   ```

2. The release key:

   ```sh
   target/release/fogata release keygen
   ```

   It asks for a passphrase (at least 16 characters, twice) and makes
   `~/.config/fogata-release/release-key` (the secret, sealed with the passphrase: Argon2id,
   then XChaCha20-Poly1305; readable only by you; never printed) and `release-key.pub`. It
   prints a line `ReleaseKey::release([0x.., ...]),` and a line starting `fogata-prerelease
   namespaces="file" ssh-ed25519`. Every signing asks for the passphrase again; without it
   the file is no key, and nothing can recover it.

3. The rescue key, on a USB stick, with a passphrase of its own (not the release key's).
   The key file must be readable by you only, which a stick formatted FAT or exFAT (as most
   come) cannot keep: format it ext4 (Linux) or APFS (macOS), or mount it with
   `fmask=077,dmask=077`. `keygen` refuses a drive where the file would show to others.
   Take the computer off the network while you make it (unplug the cable, turn off Wi-Fi),
   plug in the stick (mounted at, say, `/media/<you>/RESCUE`), and run:

   ```sh
   target/release/fogata release keygen --rescue --dir /media/<you>/RESCUE/fogata-rescue
   ```

   It makes `rescue-key` and `rescue-key.pub` on the stick and prints
   `ReleaseKey::rescue([0x.., ...]),` and its own `fogata-prerelease namespaces=...` line.
   Copy those two lines into a text file on your computer, then unmount the stick, put it
   away, and go back online. Make a second copy of the stick's `fogata-rescue` folder on
   another stick kept in another place. The rescue key never goes onto the computer's disk.

4. In `crates/fogata-update/src/lib.rs`, put both lines in `RELEASE_KEYS`:

   ```rust
   pub const RELEASE_KEYS: &[ReleaseKey] = &[
       ReleaseKey::release([0x12, 0x34, /* ...the line keygen printed... */]),
       ReleaseKey::rescue([0x56, 0x78, /* ...the line keygen --rescue printed... */]),
   ];
   ```

   Run `tools/check.sh` and commit. Only builds made from this commit on take releases.

5. The Linux Flatpak has a signing key of its own, since flatpak verifies GnuPG signatures:

   ```sh
   tools/beta/flatpak-key.sh
   ```

   makes it in `~/.config/fogata-flatpak-key/gnupg/` (GnuPG asks for a passphrase, here and
   at every release: not the release key's) and its public half as
   `~/.config/fogata-flatpak-key/flatpak-signing-key.gpg`. Every first install carries that
   public half, and an installed copy takes from its origin only what this key signed: it
   must stay the same for as long as there are pre-releases (a new one means every Linux tester installs
   again).

   This key is as strong as the release key on Linux: flatpak installs whatever commit it
   signed, newer than the one installed, from the loopback origin, through `flatpak update`
   or a software centre, with no release manifest and none of its build rules; anything
   that runs on a tester's computer can answer on that port while Fogata does not (another
   account, a Flatpak with the network). And no release can take it back: the rescue key
   does not cover it. So it lives apart from the release and rescue keys, never in their
   folders (`flatpak-key.sh`, `flatpak.sh` and `release.py` refuse a GnuPG home inside a
   folder that holds one of them, and `fogata release keygen` a folder that holds it). A key
   made before in `~/.config/fogata-release/gnupg` is moved, keeping the same key so that
   installed copies go on taking updates:

   ```sh
   mkdir -m 700 ~/.config/fogata-flatpak-key
   mv ~/.config/fogata-release/gnupg ~/.config/fogata-release/flatpak-signing-key.gpg ~/.config/fogata-flatpak-key/
   ```

6. Keep a copy of `~/.config/fogata-release` on a third USB stick, not the rescue stick, a
   copy of `~/.config/fogata-flatpak-key` on a fourth, and write the passphrases on paper
   kept somewhere else. The folders should stay out of synced and backed-up places. Whoever has a key and its passphrase can make every tester run
   their code; without the release key and the rescue key no copy takes a release again.

7. Add the two `fogata-prerelease namespaces=...` lines to `tools/allowed_signers` and
   commit them before the first pre-release. Testers take that file from the source code,
   apart from the downloads, to check the signed list of first installs with
   `ssh-keygen -Y verify ... -I fogata-prerelease` (the README of the downloads tells them
   how), and `python3 tools/release.py verify --prerelease` checks a folder of first installs
   from a checkout. The pre-release's keys are named `fogata-prerelease` and the SSH key of
   public releases `fogata-release` (docs/releasing.md), so a line that trusts one never lets
   a signature of the other pass: a pre-release key cannot vouch for a public release, nor
   the other way round. (Lines given out before the pre-release's keys took this name read
   `fogata-release`; give testers the lines again.)

`docs/releasing.md` describes public releases, which use an SSH key of their own in
`~/.config/fogata-release/ssh/`: it never replaces these keys, and `fogata release keygen`
refuses a folder that holds any key already.

## Checks before a release

Required before every release, on the commit that will be released, beside the checks of
every push (docs/ci.md). No workflow runs them: each needs a real sound server, a second
system or an hour, and each is here because it found a fault that every push check had
passed. A release waits until all of them pass; one that cannot be run is said so, by
name, in the release's notes for the maintainers, never passed over in silence.

1. **The push checks, whole**: `tools/ci.sh` says `result: pass` here, and GitHub's `CI`
   run of the commit is green in its four jobs (`linux`, `linux-checks`, `macos`,
   `windows`).
2. **Linux, the optimised build, in calls.** A build made for debugging did not crash
   where the optimised one did, so calls are run with a profile or release build:
   - a clean release build (`rm -rf app/build/flutter_assets` first in a tree that held a
     debug build, then `flutter build linux --release`) that passes
     `python3 -I -B tools/beta/ci_builds.py check-build linux app/build/linux/x64/release/bundle`;
   - the video plugin's native tests, ThreadSanitizer's among them:
     `app/third_party/flutter_webrtc/common/cpp/test/run_tests.sh`;
   - every microphone check of the call lab, `tools/call-lab/mic.sh`, on a debug build of
     the call test (`flutter build linux --debug -t integration_test/call_test.dart`;
     `media` needs that build's made-up camera);
   - calls with a camera and with a shared screen on a profile build
     (`flutter build linux --profile -t integration_test/call_test.dart`, then
     `BUNDLE=<that bundle>/fogata tools/call-lab/measure.sh 4 screen 90`, and again with
     6 copies; `measure.sh 4 camera 60` on the debug build): every copy's result says
     `passed`, no copy was kept out, and `coredumpctl list --since <the start>` shows no
     crash of `fogata`.
3. **The network.** `tools/netlab/netlab.sh`, every scenario (about 70 minutes). And on
   the real internet, the release build of the command line must log "this device's
   public address confirmed" within seconds:
   `RUST_LOG=warn,fogata_net::node::network=debug target/release/fogata --data-dir $(mktemp -d) network`.
4. **Windows, a real call.** The push checks passed while a call on Windows carried no
   sound at all. On a Windows computer (a virtual one will do, with a virtual sound
   cable for its microphone): the call test built there and run against a copy on the
   Linux computer (`integration_test/call_test.dart`, `FOGATA_TEST_MIC=system`, the
   input chosen by name with `FOGATA_TEST_INPUT`), both results `passed`;
   `tools/smoke-windows.ps1`; and, once the builds of the release exist (step 2 of "A
   release"), the attested Windows build opened there on a data folder of its own.
5. **macOS, a real call.** `tools/ci-leg.sh macos` on a Mac, and the call test between a
   copy on the Mac and one on the Linux computer. Once the builds exist, the universal
   application started under Rosetta as well
   (`arch -x86_64 Fogata.app/Contents/MacOS/Fogata`, with a `FOGATA_DATA_DIR` of its
   own): the Intel half runs nowhere else before testers have it. That is the Intel code
   under translation, not an Intel Mac; say so.
6. **What is on a tester's disk.** A copy of a data folder written by the last release
   opens with this build, with everything in it, and the test against the last release
   passes (`tools/previous-release.sh`, step 1 of "A release"), or the change that breaks
   it tells old copies to update (`release.py --min-build`, the group level notices).

A call between two real networks cannot be made from one local network: the network lab
and the public-address check of 3 stand in for it, and the notes of the release say that
it was not tried for real.

## A release

1. In `crates/fogata-update/src/lib.rs` raise `THIS_BUILD` by one and set `THIS_VERSION`
   (`0.1.0-beta.2`) and the number in `BUILD_MARKER`; in `app/pubspec.yaml` set `version` to
   the version without its `-beta...` part, `+`, and the build (`0.1.0+2`). Tests check that
   they all agree. Commit, run the checks of "Checks before a release" above on that
   commit, and `tools/ci.sh` (docs/ci.md): it must say
   `result: pass`, with the macOS and Windows legs run, or checked by hand on those systems
   at that very commit (the same commit the builds below come from: copy it there with
   `git bundle` or a push, never by copying files). Every build of the release comes from
   that commit, with no changes. Then check it against the last release: build the command
   line at the last release's tag, in a worktree and a target folder of its own, and run
   the test of the two together: the weekly workflow on GitHub does it, job
   `previous-release`, started by hand on this commit (`gh workflow run weekly.yml --repo
   holypandadev/Fogata --ref main` once the commit is the head of `main`), or here:

   ```sh
   git fetch --tags
   tools/previous-release.sh            # or: tools/previous-release.sh v0.1.0-beta.1
   ```

   It builds the command line of the last release tag before the commit (or the tag given)
   in a worktree of its own, with its target folder in `~/fogata-previous-target`, runs
   `cargo test -p fogata-core --test previous_release -- --ignored` against it, and writes
   `<git dir>/fogata-ci/<commit>.previous-release`, which says `result: pass` only when a
   test ran and passed in a checkout with no changes.

   `release.py` (step 3) refuses to make the release unless both hold for the commit
   checked out: CI passed, as GitHub's `CI` workflow says (asked with `gh run list`; when
   gh is missing or offline, or GitHub has no run of the commit, `tools/ci.sh`'s record
   here must say `result: pass`; a run on GitHub that failed or is still going refuses it
   whatever the record says), and the test against the last release passed (the job
   `previous-release` of a weekly run at this commit, or the record of
   `tools/previous-release.sh`). `--no-ci-check` and `--no-previous-check` leave each out,
   for a reason you know; `release.py` then says so in large letters.

   It must pass: what this version writes to a group, copies of the last release must take
   in whole, or a group splits between those who updated and those who did not (see
   docs/architecture.md, 3.15 and 5.1). The tests of `crates/fogata-core/tests/old_copy.rs`
   need copies from before given features, older than any published tag: run them too when
   the change touches what they cover and such a build is at hand. The `golden` tests of
   fogata-proto, fogata-state, fogata-friends and fogata-net (in the ordinary test run) hold
   the bytes of every wire message, log entry and signed record as released versions write
   them, in each crate's `tests/data`: a fixture changed since the last tag is a change of
   the wire or the log, which must have gone through a capability level
   (`fogata-proto/tests/frozen/mod.rs` says how they are written anew).
2. Build each system's application, release builds only, from that commit with no
   changes. GitHub's computers build them (`.github/workflows/release-build.yml`), so that
   anyone can check where they came from; your keys never go there, and the run holds no
   secret: you sign on your own computers, as before. Building on your own computers is the
   fallback, in 2d.

   a. Push the commit to `main` (or a branch of its own), then start the workflow on it:

      ```sh
      gh workflow run release-build.yml --repo holypandadev/Fogata --ref main \
          -f commit=$(git rev-parse HEAD)
      gh run list --repo holypandadev/Fogata --workflow release-build.yml   # its id
      gh run watch --repo holypandadev/Fogata <run id>
      ```

      It is started by hand only: the release's tag is made when the release is published
      (step 6), after these builds, so a tag does not start it. The attestations name the
      commit the run is on, so the run refuses to build when `commit` is not the head of
      `--ref` as it starts: start it again on a ref at that commit (a tag given as `--ref`
      must be `v` and `THIS_VERSION`). The macOS updater is built with your team id pinned
      into it (see 2d): give it with `-f macos_team=<team>`, or once as the repository
      variable `FOGATA_MACOS_TEAM` (Settings, Secrets and variables, Actions, Variables). It
      is no secret: every signature of yours names it. Without it the run fails in its first
      job, in seconds, before anything is built.

      The run builds Linux (`flutter build linux --release` on Ubuntu 24.04, the bundle
      zipped with its links), Windows (`flutter build windows --release`, the contents of
      `Release` zipped) and macOS (`flutter build macos --release`, universal, and
      `tools/beta/macos-updater.sh --team <team>`; signed only to run there, not with your
      identity), from a fresh clone, with no cache of an earlier run; the Mac with the
      Xcode `XCODE_VERSION` names and the CocoaPods version of `Podfile.lock`. Each job stops
      unless its library's commit marker is the commit with no changes, and unless its build
      passes what `release.py` checks before packing it (`tools/beta/ci_builds.py
      check-build`, the same code: docs/ci.md), so a build `release.py` would refuse fails
      here, and CI has made the same checks at every push. Each job writes what it built
      with (`tools/toolchain.sh`: the image, Flutter's engine, Rust, CMake, the compilers
      and SDKs) to the run's summary and as `toolchain-<system>.txt` beside its zip, outside
      it. Then a last job, which runs
      none of the build's code, lists the SHA-256 of the three zips (in the run's summary)
      and has GitHub attest each: a build provenance attestation (SLSA), signed through
      Sigstore, saying that `release-build.yml` of this repository made that file from that
      commit on GitHub's computers. The run keeps the builds 90 days (the most GitHub
      allows); the release keeps them for good (step 3).

   b. On the Linux computer, at that commit (`git status` clean), fetch and check the
      Linux and Windows builds (`gh auth login` once; reading is enough):

      ```sh
      python3 -I tools/beta/ci_builds.py fetch <run id> --only linux,windows \
          --out ~/fogata-ci-builds/0.1.0-beta.2
      ```

      It refuses a run that did not pass or built another commit than the one checked out,
      and keeps a build only when `gh attestation verify` finds GitHub's attestation of it
      (by `release-build.yml` of holypandadev/Fogata, from this commit, not on a self-hosted
      computer) and its SHA-256 is the one the run listed. It leaves
      `linux/bundle` (for `--linux-bundle`), `Fogata-windows-x64.zip`, `SHA256SUMS`, the
      attested zips themselves in `attested/`, and each build's `toolchain-<system>.txt`
      (not attested: a note of what built it). The whole folder is what `release.py
      --ci-builds` takes (step 3). A gh without `gh attestation` (older than 2.49) cannot
      fetch: then fetch all three on the Mac (2c, without `--only`) and copy that whole
      folder to the Linux computer (`rsync -a`, or `tar` and back: links and modes must
      stay).

   c. On the Mac, at the same commit, the same for macOS, then sign it there (see 2d for
      the identity and the team) and zip it:

      ```sh
      python3 -I tools/beta/ci_builds.py fetch <run id> --only macos --macos-team <team> \
          --out ~/fogata-ci-builds/0.1.0-beta.2
      tools/sign-macos.sh "<identity>" ~/fogata-ci-builds/0.1.0-beta.2/macos/Fogata.app
      ditto -c -k --keepParent ~/fogata-ci-builds/0.1.0-beta.2/macos/Fogata.app Fogata-macos-universal.zip
      ```

      `--macos-team` refuses an updater built for another team, as `sign-macos.sh` does
      again.

   What the attestations show: that the builds that go into the release are the ones
   GitHub made from the commit, which anyone can check (`gh attestation verify <zip>
   --repo holypandadev/Fogata`). `release.py --ci-builds` (step 3) refuses any build that is
   not, file for file, the attested one (the Mac app but for the signatures `sign-macos.sh`
   gives it: each Mach-O file is compared with its code signature taken off), and copies
   the attested zips into `attested/` beside the release, to upload with it. What testers
   download is made from them on your computers (the Flatpak, the Windows zip with its
   installer, the Mac app with your signature), so those files are covered by `SHA256SUMS`,
   signed by the release key, as before, not by the attestations; with the attested zips on
   the release page, anyone can check the chain: the attestation names the zip, and the
   shipped files hold what the zip holds.

   d. By hand, when GitHub cannot build (the fallback), on each system. The Rust is the one
      `rust-toolchain.toml` names (rustup installs it the first time), and the crates the
      ones `Cargo.lock` names;
      libopus is built with CMake (Windows uses Visual Studio's own; on macOS install it once
      with `brew install cmake`). The macOS build is universal (Apple silicon and Intel): add
      both Mac targets to that Rust once, `rustup target add aarch64-apple-darwin
      x86_64-apple-darwin --toolchain 1.99.1` (the version `rust-toolchain.toml` names), or the
      build stops with "the Rust target x86_64-apple-darwin is not installed".
      - Linux: `cd app && flutter build linux --release`,
        or leave it to `release.py` in the next step with `--build-linux`, which builds the
        bundle from a clean worktree of the commit, in a target folder of its own. Either way
        the build takes the library from the folder cargo built in, also with `CARGO_TARGET_DIR`
        set.
      - Windows: `flutter build windows --release`, then zip the *contents* of
        `build\windows\x64\runner\Release` (`Compress-Archive -Path Release\* -DestinationPath
        Fogata-windows-x64.zip`).
      - macOS: `flutter build macos --release`, `tools/beta/macos-updater.sh --team <team>`
        (the team ID, the OU of the certificate: `security find-certificate -c "<identity>" -p |
        openssl x509 -noout -subject` shows it as `OU=`; not the ten characters in brackets at the
        end of the identity's name, which name the certificate, not the team: the updater takes
        new versions signed by that team only), `tools/sign-macos.sh "<identity>"` (online: every
        signature gets Apple's timestamp, so it stays valid after the certificate expires),
        then
        `ditto -c -k --keepParent app/build/macos/Build/Products/Release/Fogata.app Fogata-macos-universal.zip`.
        The identity of the releases is `Apple Development: contact@holypanda.dev
        (6TZ99NVN92)`, of team `TB49TD97MX` (releases up to 0.1.0-beta.9 carry an earlier
        certificate of the same team).
        Sign every release with the same identity, or macOS forgets the permissions testers
        gave Fogata and the updater refuses the new bundle. Every signature carries the
        certificate, which anyone can read from the app (`codesign -dv --verbose=4`): an
        Apple Development certificate names the Apple Account's e-mail, and a personal
        team's the account holder's name. Moving the Apple Account to another e-mail
        changes what the next certificate names and keeps the team; only a Developer ID
        certificate of an organisation's team (a paid membership) names the organisation
        instead of a person, and a new team means every Mac tester installs again (the
        updater pins the team). An Apple Development certificate
        lasts a year: `security find-certificate -c "<identity>" -p | openssl x509 -noout
        -enddate` says when; before that day, make a new one in Xcode (same team) and sign the
        next release with it.

      Each build made by hand can be checked as `release.py` will check it before it is
      carried to the Linux computer: `python3 -I tools/beta/ci_builds.py check-build
      <linux|windows|macos> <the bundle, the Release folder or Fogata.app>`.
3. Bring the builds to the Linux computer and run (the paths of 2b; by hand, the bundle is
   `app/build/linux/x64/release/bundle`):

   ```sh
   cargo build --release -p fogata
   python3 -I tools/beta/release.py --build 2 --version 0.1.0-beta.2 --min-build 1 \
       --notes notes.txt --ci-builds ~/fogata-ci-builds/0.1.0-beta.2 \
       --linux-bundle ~/fogata-ci-builds/0.1.0-beta.2/linux/bundle \
       --windows-zip ~/fogata-ci-builds/0.1.0-beta.2/Fogata-windows-x64.zip \
       --macos-zip Fogata-macos-universal.zip --out ~/fogata-releases/0.1.0-beta.2
   ```

   `--ci-builds` names the folder `ci_builds.py fetch` made (2b): every build given must be
   the one GitHub built and attested there, file for file, and the Mac app the attested one
   but for its signatures; the attested zips must still have the SHA-256 their run listed.
   With builds made by hand (2d), or `--build-linux` in place of `--linux-bundle` (which builds the Linux bundle there, from a clean
   worktree of the commit checked out), give `--no-ci-builds` instead: `release.py` says in
   large letters that nothing was compared. It refuses to go on while `RELEASE_KEYS` lacks a release key or a rescue key (no copy would
   take the release), while the checkout has changes not committed, while nothing says the
   commit passed CI and the test against the last release (step 1; `--no-ci-check`,
   `--no-previous-check`), or while the command-line
   program that packs and signs (`target/release/fogata`, or `--fogata`) was built from
   another commit or with changes (its own commit marker says which: build it again after
   every commit), refuses debug and
   profile builds, checks that each build holds build 2, trusts exactly the keys
   `RELEASE_KEYS` names and was built from the commit checked out with no changes (each
   release build of the library carries the commit it was made from, with `+dirty` when a
   file the build reads (the Rust code, the Dart code, the runners and plugins under
   `app/linux`, `app/windows`, `app/macos` and `app/third_party`, `rust-toolchain.toml`) had
   changes or was not in git yet; a zip built before the keys were added, or from another
   commit, is refused), checks that every `.exe` and `.dll` of the
   Windows build carries ASLR, DEP, Control Flow Guard and EH continuation metadata (all but
   what `MITIGATIONS` in the script says a file cannot have), which a variable such as
   `RUSTFLAGS`, `CL` or `LINK` in the build's environment could drop, and that `fogata.exe`
   is not CET shadow stack compatible (off on purpose until Fogata is tried on a processor
   with shadow stacks; see docs/architecture.md, 5d), checks that every program and library of
   the Linux bundle carries RELRO and BIND_NOW (ours the stack protector too) and looks for
   libraries only beside it (`$ORIGIN`), and that the WebRTC plugin links PipeWire and
   PulseAudio (without `libpipewire-0.3-dev` where it is built, the plugin still builds, but
   with no screen capture under Wayland, no screen sound and no silent input), reads which
   architectures every Mach-O file of the Mac app was built for (from their headers: Linux
   has no `lipo`) and publishes a universal app, as `Fogata-<version>-macos-universal`, under
   both `macos-arm64` and `macos-x64`, one package that the manifest lists for each, since
   a copy looks only for the platform of the Mac it runs on (one built for a single
   architecture is refused unless `--macos-arch arm64` or `x64` says so), makes the Flatpak (GnuPG asks for the signing key's passphrase), packs each system's
   files, writes and signs the manifest (`release/`), and puts the first installs with a
   README into `drive/`, with `SHA256SUMS`, the SHA-256 of every file there, signed by the
   same key (`SHA256SUMS.sig`). The release key's passphrase is asked once, for both.
   The Windows zip carries `Fogata.sha256`, the SHA-256 of each of its files, and its
   installer installs those files only, each checked. `--min-build` is the oldest build that
   still works with this one: raise it only when the network protocol changes in a way
   older copies cannot follow; they are then told they must update. A release expires after
   180 days (`--days`).

   A copy takes a release of the release key only when it is newer than every build it
   runs or holds, at most 1000 builds above the build it runs, and made no earlier than the
   release it holds: raise `THIS_BUILD` by one each time.
4. Check it:

   ```sh
   target/release/fogata release verify ~/fogata-releases/0.1.0-beta.2/release
   target/release/fogata release verify-sums ~/fogata-releases/0.1.0-beta.2/drive
   ```

5. Optionally, publish it into the data folder of a copy of yours that stays online
   (close that copy first):

   ```sh
   target/release/fogata --data-dir <data folder> release publish ~/fogata-releases/0.1.0-beta.2/release
   ```

   Start the copy again: it offers the release to its friends and members, they to theirs.
   For the Flatpak the data folder is `~/.var/app/dev.holypanda.fogata/data/fogata`.
6. Make the pre-release on GitHub (tag `v0.1.0-beta.2`, marked as a pre-release, not a
   draft) and upload every file of `release/` to it, under the same names: `release.json`,
   `release.sig` and the `.pack` of each system. Every copy looks at the newest releases
   there at its start and every six hours and takes the newest one the release key signed
   (docs/architecture.md, 5f), so this is what updates everyone; friends give it on as
   well. Upload the files of `drive/` to the same pre-release, for new testers, and those of
   `attested/` (GitHub's builds the release was made from, kept there for good: the run's
   copies go after 90 days; the README tells testers they are for checking, not for
   installing). Testers check
   `SHA256SUMS.sig` with the key lines of `tools/allowed_signers` from the source code, then
   compare the checksum of their file with what their computer works out before they open
   anything (the README tells them how).

## Dates that break things

- **Each release expires 180 days after it is made** (`--days`). After that no copy takes or
  installs it, from friends or from the web; installed copies keep running. When the newest
  release has expired, a new tester cannot be brought up to date at all. So release at least
  every 150 days. beta.8 expires on 2027-05-10.
- **The Apple Development certificate expires a year after it was made** (the current one,
  `Apple Development: contact@holypanda.dev (6TZ99NVN92)` of team `TB49TD97MX`, on
  2027-11-12). Signatures made before stay valid (they are timestamped), so installed Macs
  keep running; the next release needs a new certificate of the same team (2d). If the Apple
  account or its team is lost, every Mac tester must install again: the updater and
  `Checks.swift` pin the team.
- **The Flatpak signing key and the `tools/allowed_signers` lines never expire.** Nothing
  forces a renewal; the drill below is what shows they still work.

`tools/beta/deadlines.py` lists these dates and how far each is; the weekly workflow runs it
(job `dates`, with the newest release's `release.json` read from GitHub) and fails 30 days
before any of them, with a warning from 75 days before. When a certificate is renewed, change
its line in `DEADLINES` there, in the same commit as anything else it changes.

## Twice a year: check that the keys still open

Every key is useless without its file and its passphrase, and nothing else ever opens the
backups: check each one, by hand, every six months (and after moving house, a new computer, a
stick that was dropped). Each check signs a list of one throwaway file, which says nothing
about any release, and checks it with the keys compiled into `fogata`.

1. At the commit of the last release, build the command line (`cargo build --release -p
   fogata`): its `RELEASE_KEYS` are the keys copies trust.
2. For each copy of each key: the release key in `~/.config/fogata-release`, its backup stick
   (keys section, step 6), and both rescue sticks (step 3, offline, as when it was made). Plug
   the stick in, then, with `<key>` the key file (`release-key` or `rescue-key`) and the
   passphrase typed from the paper copy, not from memory:

   ```sh
   drill=$(mktemp -d)
   echo "key drill $(date -u +%F)" > "$drill/drill.txt"
   target/release/fogata release sign-sums --key <key> "$drill"
   target/release/fogata release verify-sums "$drill"
   rm -rf "$drill"
   ```

   `verify-sums` must say `SHA256SUMS: signed with the release key <hex>` (or `rescue key`)
   and `drill.txt: good`. A passphrase refused, a file that does not open, or a signature
   `verify-sums` does not take (a key that is not the one compiled in) means that copy is
   lost: make a new copy from one that passed, or follow "If the release key leaks or is
   lost" (or the rescue key's section) while the other copies still open.
3. The Flatpak signing key, in `~/.config/fogata-flatpak-key/gnupg` and on its backup stick
   (step 6), with its passphrase from the paper:

   ```sh
   echo drill | gpg --homedir <gnupg folder> --clearsign > /dev/null && echo opens
   gpg --homedir <gnupg folder> --list-secret-keys --with-fingerprint
   ```

   It must say `opens`, and the fingerprint must be the one of
   `~/.config/fogata-flatpak-key/flatpak-signing-key.gpg` (`gpg --show-keys --with-fingerprint`
   on that file).
4. Unmount the sticks and put them back. Note the day in `published.log` beside the release
   key (`echo "$(date -u +%FT%TZ) key drill: all copies open" >> ~/.config/fogata-release/published.log`).

## If the release key leaks or is lost

Leaked means anyone else may have the secret file and its passphrase (the computer was
broken into, the file and the paper went missing together); lost means you no longer have
the file or its passphrase. Both are mended the same way, with the rescue key.

1. Stop signing with the old key. Move its two files out of the way and make a new one:

   ```sh
   mkdir ~/fogata-old-release-key
   mv ~/.config/fogata-release/release-key ~/.config/fogata-release/release-key.pub ~/fogata-old-release-key/
   target/release/fogata release keygen
   ```

   (If the whole computer was broken into, the Flatpak signing key may have leaked too: see
   the next section.)
2. In `crates/fogata-update/src/lib.rs`, replace the old `ReleaseKey::release(...)` line
   with the new one and keep the `ReleaseKey::rescue(...)` line. Set `THIS_BUILD` to the last
   build you released plus 1001 (last build 7: 1008), with `THIS_VERSION`, `BUILD_MARKER` and
   `app/pubspec.yaml` as for any release. Commit and build every system.
3. Plug in the rescue stick and make the release with the rescue key (its passphrase is
   asked):

   ```sh
   python3 -I tools/beta/release.py --key /media/<you>/RESCUE/fogata-rescue/rescue-key \
       --build 1008 --version 0.1.0-beta.8 --min-build 1 ... --out ~/fogata-releases/0.1.0-beta.8
   ```

   Unmount the stick and put it away. Check and publish it as any release (steps 4 to 6).
   `release verify` says it is signed with the rescue key.
4. Copies that took a release the leaked key signed (at most 1000 builds above their own)
   take this one, which is numbered above it, and once installed trust only the new release
   key. A copy more than 999 builds behind cannot take it and must install again from
   `drive/`. A copy that already *installed* a version the leaked key signed runs that
   version's code, which nothing it receives can undo: if any tester's Fogata shows a
   build you did not release, they uninstall it and install again from `drive/`, checking
   the checksum.
5. Give testers the new release key's `fogata-prerelease namespaces=...` line (and the rescue
   key's, again) for their `allowed_signers`. Later releases are signed with the new release
   key as usual (`THIS_BUILD` 1009, then 1010...).

## If the Flatpak signing key leaks

Whoever has it and its passphrase can make a Linux copy install their code wherever they can
answer on the loopback port (any program on the tester's computer), and nothing a release
does takes that back: a summary signed by a new key can only add keys to those a copy
trusts. Make a new one (`tools/beta/flatpak-key.sh`, after moving
`~/.config/fogata-flatpak-key` aside), release as usual, and have every Linux tester remove
Fogata (`flatpak uninstall --user dev.holypanda.fogata`, which keeps the data) and install
again from `drive/`, checking the checksum.

## If the rescue key leaks or is lost

Make a new rescue key on a new stick (keys section, step 3), replace the
`ReleaseKey::rescue(...)` line in `RELEASE_KEYS`, and release as usual with the release key.
Copies trust the new rescue key once they install that release. Until every copy has it, a
leaked rescue key can make a release that every copy still takes: there is no third key, so
the rescue key stays offline and is used for nothing but this.

## Replacing a release key that did not leak

Release a build whose `RELEASE_KEYS` has the old and the new `ReleaseKey::release` lines,
signed with the old key; sign the next releases with the new one and drop the old line a
release later.

## Rehearsing on macOS

Before the first release, and after any change to the updater, rehearse the three things a
Mac tester goes through, with the signing identity of the releases, on a Mac user that never
ran Fogata (a user of its own: Fogata's container is per user, and every copy of
`dev.holypanda.fogata` that user runs shares it). `tools/beta/rehearse-macos.sh` builds and
checks each step; it signs only in `build`.

1. `tools/beta/rehearse-macos.sh prereqs`, as that user.
2. Two releases, A and B (build N and N+1), each from its own commit, as in "A release":
   `tools/beta/rehearse-macos.sh build <build> <version> "<identity>" <team> <out>` on the
   Mac for each, then `release.py` with the release key on the Linux computer.
3. **A clean install of A** from its `drive/` folder: check the SHA-256 as the README says,
   unzip, drag Fogata to `~/Applications`, open it, "Open Anyway" in Privacy & Security, give
   it the microphone and camera in a call. Then `rehearse-macos.sh installed <N> <team>`.
4. **An update from A to B**: publish B into the data folder of a copy online that is a
   friend of the rehearsal's (on another computer); the Mac copy takes it and asks to
   restart. Then `rehearse-macos.sh updated <N+1> <team>`, and in Fogata: the same profile,
   friends and groups, and no new question for the microphone or the camera.
5. **The rescue**: a release C made with the rescue key (build N+1001, "If the release key
   leaks or is lost"), published the same way; the copy takes it, and `rehearse-macos.sh
   updated <N+1001> <team>` passes. A release of the old release key after it is refused.

Each step that fails stops the rehearsal: the release waits until it passes.

## The Linux Flatpak

`tools/beta/flatpak.sh` builds it with the `flatpak` command alone (no flatpak-builder, no
SDK), copying the release bundle into `/app` on the `org.freedesktop.Platform` 26.08 runtime,
and signs the repository's commit and summary with the Flatpak signing key. The first
install's origin is `http://127.0.0.1:47811/`, where the installed application serves an
update while it installs; because the install carries the key, flatpak takes from that
address only a summary and a commit the key signed, and no commit older than the one
installed, whoever answers there; but whatever newer commit the key signed, it takes
without any release manifest (keys section, step 5). `tools/beta/test-flatpak-origin.sh`
tries this in a flatpak installation of its own (`--unsigned` makes a Flatpak that verifies
nothing, for the lab's tests only: never give one to a tester).
The bundle is built on Ubuntu 24.04 by the release workflow (glibc 2.39), or by hand on this
computer (Ubuntu 26.04, glibc 2.43); the runtime's glibc (2.44) is newer than both, and the
script refuses a bundle that needs a library the runtime lacks.

The tray needs no library the runtime lacks: Fogata serves its icon and menu over D-Bus
itself (crates/fogata-tray), so the Flatpak only talks to `org.kde.StatusNotifierWatcher`.
For the Away status it asks the compositor over the Wayland socket it already has, the X
server over the X11 one (MIT-SCREEN-SAVER, with the runtime's libXss), and
`org.gnome.Mutter.IdleMonitor` (idle time on GNOME) how long the computer has gone unused.
It does not get `org.freedesktop.ScreenSaver`, whose Lock, SetActive and Inhibit flatpak
cannot keep from it, so it does not know when the screen is locked: a key pressed at the
lock screen counts as someone there. Cameras come through the Camera portal and PipeWire,
so the Flatpak has no `--device=all`, only `--device=dri` (the GPU): the first time the
camera's choice shows or a call turns the camera on, the system asks the person whether
Fogata may use the cameras, once (GNOME's and KDE's settings, or `flatpak permission-reset
dev.holypanda.fogata`, change the answer later). Refused, or on a system without the portal's
Camera interface, the settings and the call say so. A camera chosen in an earlier Flatpak
stays chosen (its id is its V4L2 bus either way). `tools/beta/test-flatpak-camera.sh` tries
it in a Flatpak with these permissions, in a session of its own with a made-up camera.

The data key is not kept in the Secret Service: `--talk-name=org.freedesktop.secrets` would
let the application read every secret the person keeps there. It is sealed with the secret
the Secret portal gives Fogata alone (docs/architecture.md, 5c). A Flatpak built before kept
its key in the Secret Service, and one without the permission cannot read it there: on the
first start after installing this one over such a copy, Fogata says the key is out of
reach and how to let it be reached once,

```sh
flatpak override --user --talk-name=org.freedesktop.secrets dev.holypanda.fogata
```

then starts as before, with the key moved, after which

```sh
flatpak override --user --no-talk-name=org.freedesktop.secrets dev.holypanda.fogata
```

takes the permission back (`tools/beta/test-flatpak-keychain.sh` goes through all of it with
fogata-core's own code in a Flatpak of these permissions, in a session and flatpak
installation of its own; run it, and `tools/beta/test-flatpak-origin.sh`, after any change to
`flatpak.sh`, the vault or the update path). Nothing is lost on the way: the key stays in the Secret Service
too (under the service `Fogata`), and a move cut short is made again at the next start. Once
Fogata opens without the permission, that copy can be removed from the keychain
(Seahorse, KWalletManager).

Sound goes through the PulseAudio socket, and two things need PipeWire's own
(`--filesystem=xdg-run/pipewire-0`), which flatpak does not give with it: the sound of a
shared screen, which links itself to every other application's output and leaves out
Fogata's own, and the silent input WebRTC records from while the microphone is off and that
sound goes out. Without the socket the first takes everything the default output plays,
the call's voices included, and the second leaves the microphone open. PipeWire marks the
application as a Flatpak client, and WirePlumber's default rules give it read and execute
permissions on the session's nodes: it sees and links other applications' streams, as the
PulseAudio socket already lets it record any of them.

Building from source inside the SDK instead (flatpak-builder) would need `flatpak-builder`
(apt) and the SDK extensions for Rust and LLVM; the SDK itself (`org.freedesktop.Sdk` 26.08)
is installed already.

## Testers

The README in `drive/` says it all: first check the signed `SHA256SUMS` and compare the
download's SHA-256 with its line; then Windows asks once through SmartScreen ("More info", "Run anyway"), macOS once
through Privacy & Security ("Open Anyway"), and Linux installs with `flatpak install --user`.
The first update on Linux makes the system ask whether Fogata may update itself. Authenticode
signing on Windows and notarization on macOS, which would make those warnings go, need paid
certificates and are not done.
