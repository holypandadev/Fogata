# Releasing Fogata

This is the plan for full releases, which do not exist yet: pre-releases are released as
`docs/releasing-beta.md` says, with keys made by `fogata release keygen`. The SSH key
here is another key, kept in its own folder (`~/.config/fogata-release/ssh/`) so that making
one never overwrites the pre-releases' release key (`~/.config/fogata-release/release-key`).

Every release carries `SHA256SUMS` (the SHA-256 of each package) and `SHA256SUMS.sig`, its
signature by the release key, an SSH key kept by the person who publishes releases and never
put in the repository. Release tags are signed with the same key. Anyone can check a download
with the public key in `tools/allowed_signers` and the `ssh-keygen` that comes with Linux,
macOS and Windows 10 and later (OpenSSH 8.1 or newer). No paid certificate is involved.

## The release key (once)

Make it outside the repository, readable only by you:

```sh
mkdir -p ~/.config/fogata-release/ssh
chmod 700 ~/.config/fogata-release ~/.config/fogata-release/ssh
ssh-keygen -t ed25519 -C fogata-release -f ~/.config/fogata-release/ssh/release-key
chmod 600 ~/.config/fogata-release/ssh/release-key
```

`ssh-keygen` asks for a passphrase: choose a long one; it is asked every time the key signs.
Keep a copy of `release-key` and the passphrase somewhere safe and offline. Whoever has both can
sign releases as Fogata; without them, no release can be signed with this key again.

Then add the public half to `tools/allowed_signers`, check its fingerprint, and commit it:

```sh
echo "fogata-release namespaces=\"file,git\" $(cat ~/.config/fogata-release/ssh/release-key.pub)" >> tools/allowed_signers
ssh-keygen -lf ~/.config/fogata-release/ssh/release-key.pub
```

The line names the key `fogata-release` and lets it sign files (`SHA256SUMS`) and git tags.
The pre-release keys are named `fogata-prerelease` in the same file
(docs/releasing-beta.md): `-I fogata-release`, `tools/release.py sign` and
`tools/release.py verify` take a signature of this key only, never one of a pre-release key.

## Building the packages

Only release builds are handed out: debug builds carry the Dart program for a JIT and a
debugger, and on macOS debug and profile builds run outside the App Sandbox
(`app/macos/Runner/DebugProfile.entitlements`). In `app/`:

- Linux: `flutter build linux --release`, then
  `tar -czf Fogata-<version>-linux-x64.tar.gz -C build/linux/x64/release bundle`.
  On the computers that run it, the app's own sounds (someone coming in, a message, a
  ringing call) need ALSA's plugin for the sound server: `pipewire-alsa` with PipeWire, or
  PulseAudio's ALSA plugin (`libasound2-plugins` on Debian and Ubuntu,
  `alsa-plugins-pulseaudio` on Fedora, `pulseaudio-alsa` on Arch). Desktop installs usually
  have it; without it the sounds fail while a call holds the sound card, and the log says
  which package is missing. A package for a distribution should depend on it.
- Windows: `flutter build windows --release`, then zip `build\windows\x64\runner\Release`
  as `Fogata-<version>-windows-x64.zip`.
- macOS: `flutter build macos --release` builds for Apple silicon and Intel in one bundle,
  so the Rust `rust-toolchain.toml` names needs both Mac targets, added once with
  `rustup target add aarch64-apple-darwin x86_64-apple-darwin --toolchain 1.99.1` (that
  file's version; else the build stops with "the Rust target x86_64-apple-darwin is not
  installed"). Then sign it with your signing identity (the names are listed by
  `security find-identity -v -p codesigning`; an "Apple Development" certificate is free
  with an Apple Account):

  ```sh
  tools/sign-macos.sh "Apple Development: <name> (<id>)"
  ditto -c -k --keepParent app/build/macos/Build/Products/Release/Fogata.app Fogata-<version>-macos.zip
  ```

  The release build runs with the hardened runtime, which loads only code signed by the
  app's own team: until `tools/sign-macos.sh` has signed every framework in it with the same
  identity, it does not start. The script signs with `Release.entitlements` and stops if the
  app could be debugged, could run JIT code, could load other teams' libraries, or is not
  sandboxed. Signing again with the same certificate keeps the app's designated requirement,
  so the microphone, camera and screen permissions people gave it stay.

## Licence notices

`THIRD-PARTY-NOTICES`, at the root of the repository, lists the third-party code the packages
carry (fonts, WebRTC and Flutter, the Rust crates of the Fogata library and the Dart packages)
with the text of each licence. It is made by a script from what is on the computer, without
downloading anything. Before each release, and whenever a dependency changes, run in the
repository:

```sh
python3 tools/third_party_notices.py
```

and commit the file if it changed (`python3 tools/third_party_notices.py --check` only says
whether it is up to date). The script needs `cargo` and `flutter` on the `PATH`, the crates in
Cargo's registry and the packages in the pub cache (they are there after a build), and
`libwebrtc` unpacked by a Linux build of the app. Each package also carries Flutter's own
notices, for the engine and the Dart packages, in `flutter_assets/NOTICES.Z`.

## Signing

Put the packages, and nothing else, into one folder, then:

```sh
python3 tools/release.py sign --key ~/.config/fogata-release/ssh/release-key <folder>
```

It refuses an archive that holds a debug or a profile build (whose Dart VM service lets any
program on the computer run code in the app) and stops while `tools/allowed_signers` holds no
key. It writes `SHA256SUMS`, asks for the key's passphrase to write `SHA256SUMS.sig`, and
checks the signature against `tools/allowed_signers`. Publish the packages with both files.

## Signed tags

Once per clone:

```sh
git config gpg.format ssh
git config user.signingkey ~/.config/fogata-release/ssh/release-key
git config gpg.ssh.allowedSignersFile "$PWD/tools/allowed_signers"
```

Then for each release:

```sh
git tag -s v<version> -m "Fogata <version>"
git tag -v v<version>
```

`git tag -v` prints `Good "git" signature for fogata-release`.

## Checking a download

Save the line from `tools/allowed_signers` as a file named `allowed_signers` next to the
downloaded package, `SHA256SUMS` and `SHA256SUMS.sig`. Then, on Linux or macOS:

```sh
ssh-keygen -Y verify -f allowed_signers -I fogata-release -n file -s SHA256SUMS.sig < SHA256SUMS
grep ' Fogata-<version>-linux-x64.tar.gz$' SHA256SUMS | sha256sum -c
```

On macOS use `shasum -a 256 -c` in place of `sha256sum -c`. On Windows, in PowerShell:

```powershell
cmd /c "ssh-keygen -Y verify -f allowed_signers -I fogata-release -n file -s SHA256SUMS.sig < SHA256SUMS"
Get-FileHash Fogata-<version>-windows-x64.zip
```

and compare the hash with the package's line in `SHA256SUMS`. The first command must print
`Good "file" signature for fogata-release`; anything else means the files were not published
with the release key. From a checkout, `python3 tools/release.py verify <folder>` does both
checks for every package in the folder.
