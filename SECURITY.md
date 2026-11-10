# Security policy

Fogata carries people's messages, calls and files between their own devices, and installs its
own updates. A flaw in any of that matters to everyone who runs it, so please report one
privately first, and give the project time to ship a fix before you publish it.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting: on the repository's **Security** tab, choose
**Report a vulnerability**. Only the maintainers see the report, and the fix can be prepared
in a private fork with you.

Please include what you found, what an attacker needs for it (another member of a group, a
friend, anyone on the local network, anyone on the internet, a local program), the version
(at the bottom of Settings) and the system, and a way to reproduce it if you
have one. Do not include anyone's real messages, keys or data folders.

## What to expect

- An answer that the report arrived, and a first assessment, as soon as the maintainers can;
  Fogata is maintained by one person, so it may take some days.
- A fix in a new pre-release. Copies update themselves (docs/architecture.md, 5f), so a fix
  reaches people without anything to do on their side.
- Credit in the release notes, unless you prefer otherwise.

## Scope

In scope: the application and the command-line program in this repository, the release and
update tooling (`tools/beta`, `fogata release`), and Fogata's own changes to the code it
vendors (`vendor/`, `app/third_party`). The design and its threat model are in
[docs/architecture.md](docs/architecture.md) (section 7); a way around what it says Fogata
protects is in scope too.

Reports about the components Fogata builds on (libwebrtc, Flutter, GStreamer, the operating
systems' media frameworks) are welcome; their own projects fix them, and Fogata moves to their
fixed versions (docs/native-components.md).

Only the newest pre-release gets fixes.
