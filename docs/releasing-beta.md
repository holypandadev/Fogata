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

## A release

1. In `crates/fogata-update/src/lib.rs` raise `THIS_BUILD` by one and set `THIS_VERSION`
   (`0.1.0-beta.2`) and the number in `BUILD_MARKER`; in `app/pubspec.yaml` set `version` to
   the version without its `-beta...` part, `+`, and the build (`0.1.0+2`). Tests check that
   they all agree. Commit, and run `tools/ci.sh` on that commit (docs/ci.md): it must say
   `result: pass`, with the macOS and Windows legs run, or checked by hand on those systems
   at that very commit (the same commit the builds below come from: copy it there with
   `git bundle` or a push, never by copying files). Every build of the release comes from
   that commit, with no changes. Then check it against the last release: build the command
   line at the last release's tag, in a worktree and a target folder of its own, and run
   the test of the two together (the weekly workflow on GitHub does the same, job
   `previous-release`; it must have passed for this commit or be run here):

   ```sh
   git fetch --tags
   git worktree add --detach ../fogata-previous v0.1.0-beta.1   # the last release's tag
   (cd ../fogata-previous && CARGO_TARGET_DIR=$HOME/fogata-previous-target \
       cargo build --release --locked -p fogata)
   FOGATA_OLD_CLI=$HOME/fogata-previous-target/release/fogata \
       cargo test -p fogata-core --test previous_release -- --ignored
   git worktree remove ../fogata-previous
   ```

   It must pass: what this version writes to a group, copies of the last release must take
   in whole, or a group splits between those who updated and those who did not (see
   docs/architecture.md, 3.15 and 5.1). The tests of `crates/fogata-core/tests/old_copy.rs`
   need copies from before given features, older than any published tag: run them too when
   the change touches what they cover and such a build is at hand.
2. Build on each system, release builds only. The Rust is the one `rust-toolchain.toml`
   names (rustup installs it the first time), and the crates the ones `Cargo.lock` names;
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
     `ditto -c -k --keepParent app/build/macos/Build/Products/Release/Fogata.app Fogata-macos-arm64.zip`.
     Sign every release with the same identity, or macOS forgets the permissions testers
     gave Fogata and the updater refuses the new bundle. An Apple Development certificate
     lasts a year: `security find-certificate -c "<identity>" -p | openssl x509 -noout
     -enddate` says when; before that day, make a new one in Xcode (same team) and sign the
     next release with it.
3. Bring the zips to the Linux computer and run:

   ```sh
   cargo build --release -p fogata
   python3 -I tools/beta/release.py --build 2 --version 0.1.0-beta.2 --min-build 1 \
       --notes notes.txt --linux-bundle app/build/linux/x64/release/bundle \
       --windows-zip Fogata-windows-x64.zip --macos-zip Fogata-macos-arm64.zip \
       --out ~/fogata-releases/0.1.0-beta.2
   ```

   (`--build-linux` in place of `--linux-bundle` builds the Linux bundle there, from a clean
   worktree of the commit checked out.) It refuses to go on while `RELEASE_KEYS` lacks a release key or a rescue key (no copy would
   take the release), while the checkout has changes not committed, or while the command-line
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
   with shadow stacks; see docs/architecture.md), makes the Flatpak (GnuPG asks for the signing key's passphrase), packs each system's
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
   well. Upload the files of `drive/` to the same pre-release, for new testers. They check
   `SHA256SUMS.sig` with the key lines of `tools/allowed_signers` from the source code, then
   compare the checksum of their file with what their computer works out before they open
   anything (the README tells them how).

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
The bundle is built on this computer; the runtime's glibc (2.44) is newer than Ubuntu
26.04's (2.43), and the script refuses a bundle that needs a library the runtime lacks.

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
