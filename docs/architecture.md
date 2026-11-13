# Fogata architecture

Status: pre-release. Sections marked "later" are not implemented yet.

**Start here.** Section 1 (the concepts) and section 2 (where the code is, and which crate may
use which) are the map; section 7 is the threat model in short. Then read what your change
touches: the network (3), calls (4), the group log and its rules (5.1 to 5.3), the
application core (5c) and the app (5d). Each section names its crate and files. How a change
is checked before it is pushed is in [ci.md](ci.md); fuzzing in [fuzzing.md](fuzzing.md);
releases in [releasing-beta.md](releasing-beta.md). The sections are named by number and
letter (5b, 5c...) and other documents and code comments refer to them by those names, so new
sections take new letters rather than renaming old ones.

**Where each section is.** This page holds sections 1, 2, 6 and 7. Every other section is a
file of [architecture/](architecture/), named by its number, with its number and headings
as before: a reference such as "docs/architecture.md, 3.15" or "(5c)" is found through the
contents below.

**Contents**

1. [Concepts](#1-concepts)
2. [Repository layout](#2-repository-layout),
   [the crates and their layers](#21-the-crates-and-their-layers)
3. [Connectivity](architecture/3-connectivity.md#3-connectivity):
   [rules](architecture/3-connectivity.md#31-rules),
   [network stack](architecture/3-connectivity.md#32-network-stack-fogata-net),
   [finding each other](architecture/3-connectivity.md#33-finding-each-other),
   [connecting](architecture/3-connectivity.md#34-connecting),
   [results in the network lab](architecture/3-connectivity.md#35-results-in-the-network-lab),
   [membership proof](architecture/3-connectivity.md#36-membership-proof),
   [group messages](architecture/3-connectivity.md#37-group-messages),
   [rooms](architecture/3-connectivity.md#38-rooms),
   [presence](architecture/3-connectivity.md#39-presence),
   [files](architecture/3-connectivity.md#310-files),
   [direct calls](architecture/3-connectivity.md#311-direct-calls),
   [network check](architecture/3-connectivity.md#312-network-check),
   [direct messages](architecture/3-connectivity.md#313-direct-messages),
   [typing indicators](architecture/3-connectivity.md#314-typing-indicators),
   [capability levels](architecture/3-connectivity.md#315-capability-levels)
4. [Voice and video](architecture/4-voice-and-video.md#4-voice-and-video):
   [topology](architecture/4-voice-and-video.md#41-topology),
   [transport and encryption](architecture/4-voice-and-video.md#42-transport-and-encryption),
   [calls in rooms](architecture/4-voice-and-video.md#43-calls-in-rooms),
   [realistic limits](architecture/4-voice-and-video.md#44-realistic-limits)
5. [Group management](architecture/5-group-management.md#5-group-management-fogata-state):
   [the group log](architecture/5-group-management.md#51-the-group-log),
   [the ladder](architecture/5-group-management.md#52-the-ladder),
   [invites](architecture/5-group-management.md#53-invites),
   [epochs and keys](architecture/5-group-management.md#54-epochs-and-keys),
   [MLS](architecture/5-group-management.md#55-mls-in-progress)
   - 5b. [Profiles and friends](architecture/5b-profiles-and-friends.md#5b-profiles-and-friends-fogata-friends)
   - 5c. [The application core](architecture/5c-application-core.md#5c-the-application-core-fogata-core)
   - 5d. [The desktop application](architecture/5d-desktop-application.md#5d-the-desktop-application-app-fogata-ffi)
   - 5e. [Moving an account to another computer](architecture/5e-moving-an-account.md#5e-moving-an-account-to-another-computer-fogata-core-transfer)
   - 5f. [Updates of pre-releases](architecture/5f-updates.md#5f-updates-of-pre-releases-fogata-update-fogata-net-updates-fogata-core-updates)
   - 5g. [Copies that restore an account without the keychain](architecture/5g-recovery-copies.md#5g-copies-that-restore-an-account-without-the-keychain-fogata-core-recovery)
6. [Cryptography](#6-cryptography-fogata-crypto)
7. [Threat model summary](#7-threat-model-summary)
8. [Testing](architecture/8-testing.md#8-testing)
9. [The first milestone (done): the CLI prototype](architecture/9-first-milestone.md#9-the-first-milestone-done-the-cli-prototype)

Fogata is a text, voice and video chat for small groups of friends. Every device talks
directly to the others. There are no accounts, no Fogata servers and no third-party relays.
The only public infrastructure used is the IPFS DHT, and only as a phone book: devices look
up where their friends are, then talk to them directly.

## 1. Concepts

| Concept  | Lifetime   | Stored where                     | Contents |
|----------|------------|----------------------------------|----------|
| Device identity | Permanent | Device disk, sealed with a data key the OS keychain keeps (5c) | Ed25519 key pair. Signs everything the device says. Never published. |
| Transport identity | One run of the app | RAM | Ed25519 key pair used as the libp2p `PeerId`. A new one each time the app starts, never written down; it stays the same while the app runs (also across key changes and network changes), so the network can link what one run announces, not one run to the next. |
| Friend   | Permanent  | Device disk                      | Device public key, chosen nickname, last known addresses. |
| Group    | Permanent  | Each member's disk               | Group secret, epoch, signed member list. Never messages. |
| Room     | Ephemeral  | RAM of the members present       | Chat log, participants, shared file index, media topology. |
| File     | Ephemeral  | Disk of whoever shared/downloaded it | Served on demand while the holder is in the room. |
| Direct chat | Until deleted, or its timer runs out | Both friends' disks | Messages and attachments between two friends, over a group of the two (3.13). |

Platforms: Linux, Windows and macOS until the desktop app is complete; iOS and Android after.

On desktop the app keeps running in the system tray after its window is closed, so friends
can still call and connections to them stay warm. The tray icon (Windows' notification area,
a StatusNotifier host on Linux, macOS's menu bar) opens the window on a click, its menu
opens the window or quits, and a call ringing while the window is hidden brings it back. It
is on by default; Settings turns it off, and closing the window then quits. Later: it tells
the user when friends open a room.

Windows and macOS draw the icon with the tray_manager plugin (our copy in
`app/third_party/tray_manager`, without its Linux part). On Linux the application serves
the icon itself, from Rust (`crates/fogata-tray`, called through the FFI's `show_tray`),
so it links none of the indicator libraries (libayatana-appindicator and libdbusmenu are
GPL and LGPL): a `org.kde.StatusNotifierItem` at `/StatusNotifierItem` and its menu as a
`com.canonical.dbusmenu` at `/MenuBar`, under the connection's unique name, registered
with `org.kde.StatusNotifierWatcher`. The icon is the picture itself (`IconPixmap`, from
`assets/tray/tray.png`), which a host outside a Flatpak sandbox can show; so inside a
Flatpak the item needs only `--talk-name=org.kde.StatusNotifierWatcher`. A click on the
icon (`Activate`) opens the window and the menu opens from the other button. When the
watcher's name gets a new owner (Plasma starting again), the item registers again; should
the icon's events end or fail in the window, closing the window quits from then on and a
window hidden in the tray comes back. The icon
is made only where the desktop shows tray icons, that is where a StatusNotifier host is
registered with the watcher: elsewhere (GNOME without its AppIndicator extension, an X11
desktop with only an old XEmbed tray, no session bus) closing the window quits, and
Settings says why.

One device is one identity for now. Later: several devices linked to one identity. An
identity can move to another computer whole (5e); the old computer then stops using it.

Licensing: Fogata is licensed under the GNU Affero General Public License, version 3 only
(`LICENSE`). Dependencies must use permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib and
similar), which it can take in. `cargo deny check licenses` enforces it in the pre-commit checks,
with `bans` (no wildcard versions; duplicate versions only reported), `sources` (crates.io
only) and, with the RustSec database fetched by hand, `advisories`; native components and how
to follow their security releases are in `native-components.md`.

## 2. Repository layout

```
Fogata/
  Cargo.toml              Rust workspace
  deny.toml               Licence policy
  crates/
    fogata-crypto/        Device identity, group keys, sealed messages, membership proofs. No I/O.
    fogata-proto/         Wire messages and their versioning. No I/O.
    fogata-net/           libp2p swarm: transports, discovery, NAT traversal, member and
                          friend protocols.
    fogata-cli/           The command-line program (`fogata`, docs/cli.md): a client on
                          fogata-core, the release tooling (`fogata release`), and the
                          network lab's peers.
    fogata-state/         Group log and the rules that apply it: members, roles, key epochs,
                          the chain of MLS epochs, and a voice room's state (room.rs).
                          `before/` is the crate as it was at level one, for its tests (5.1).
                          No I/O.
    fogata-mls/           MLS (RFC 9420) for group keys: its own crypto provider, a device's
                          MLS state for a group, the epoch secrets it exports (5.5). Not used
                          by the application yet. No I/O.
    fogata-friends/       Profiles, friend secrets, invite links and codes, friend handshakes.
                          No I/O.
    fogata-core/          What the application builds on: storage, one node for every group
                          and friend, views and events for the screens (5c). The CLI runs
                          on it too.
    fogata-media/         Media transport probes and the WebRTC bridge; later selective
                          forwarding on the host.
    fogata-denoise/       Noise suppression of the microphone in calls: RNNoise (nnnoiseless,
                          BSD-3-Clause, notice in its NOTICE file).
    fogata-voice/         Voice messages: Opus (libopus, BSD-3-Clause, built from source and
                          linked statically) in Ogg files written and read by its own code,
                          their levels, and playing them faster (WSOLA). No I/O.
    fogata-tray/          The tray icon on Linux: a StatusNotifierItem and its dbusmenu,
                          served on the session bus with zbus.
    fogata-update/        Pre-releases: the signed manifest and the rules for taking one,
                          the release key's files, the package format (5f). No network.
    fogata-ffi/           The flutter_rust_bridge surface over fogata-core.
  app/                    The Flutter application.
  vendor/                 Crates Fogata keeps a fork of (libp2p-quic, libp2p-mdns,
                          libp2p-identify, libp2p-kad, libp2p-stream, turn,
                          spake2), each with a FOGATA.md of its changes.
  fuzz/                   The libFuzzer targets, a workspace of their own (docs/fuzzing.md).
  tools/check.sh, ci.sh   The checks before every commit and every push (docs/ci.md).
  tools/netlab/           Simulated home and carrier NATs in Linux network namespaces.
  tools/release.py        Signed checksums of a release's packages (docs/releasing.md).
  tools/beta/             Pre-releases: the Flatpak, the release and first installs,
                          the macOS updater helper, Windows' install script
                          (docs/releasing-beta.md).
  docs/                   Architecture, threat model, decision records.
```

### 2.1 The crates and their layers

Each crate of the workspace has a layer and uses only crates below it; `tools/check.sh` runs
`tools/crate_layers.py`, which reads `cargo metadata` and fails on any dependency between
the workspace's crates that its list does not allow. A new edge is a change of that list,
made on purpose and said why, never only a line in a `Cargo.toml`.

| Layer | Crates | What they are |
|---|---|---|
| 0 | fogata-crypto, fogata-voice, fogata-denoise, fogata-tray | Pieces of their own, on nothing of Fogata's |
| 1 | fogata-proto, fogata-state, fogata-friends, fogata-update | What is said and kept: wire formats, the group log, friends, releases (each on fogata-crypto) |
| 2 | fogata-mls | MLS for group keys, on fogata-state's chain |
| 3 | fogata-net | The node: transport, discovery, groups, friends, files and updates over the network |
| 4 | fogata-media | Calls, on the node |
| 5 | fogata-core | The application's logic and storage, on everything above |
| 6 | fogata (fogata-cli), fogata-ffi | The programs: the command line and the bridge of the Flutter app |

Tests may also use a crate of their own layer (fogata-state's compare it with its frozen
copy, `fogata-state-before`) or further down, never one above. Two edges are there for
history rather than by design and should not grow: the programs reach crates around
fogata-core (the command line seven of them, the bridge six; the list in
`tools/crate_layers.py` names each), and fogata-net holds more than a transport (files, groups,
friends, strangers, rooms: 3.2), which a split into a transport crate and a node crate would
make clearer.

## 6. Cryptography (fogata-crypto)

| Purpose | Primitive |
|---------|-----------|
| Device identity, signatures | Ed25519 |
| Group key schedule | HKDF-SHA256 from the epoch secret and group id: `auth_key`, `message_key`, rendezvous keys |
| Membership proof | HMAC-SHA256 + Ed25519 signature |
| Group messages | XChaCha20-Poly1305, then Ed25519 signature |
| Sealing secrets to a member | X25519 (converted from the Ed25519 device key) + HKDF-SHA256 + XChaCha20-Poly1305 |
| Friend hellos, link requests, probe tags | HMAC-SHA256 keyed by a friend secret or link token, bodies sealed to the device key |
| Friend codes | Argon2id (64 MiB, 3 passes), then SPAKE2 (Ed25519 group) and XChaCha20-Poly1305 |
| File ids and integrity | BLAKE3 of the whole file, checked after the download |
| Data at rest | XChaCha20-Poly1305 with a random 32-byte data key kept by the OS keychain: whole files, per-record journals, 64 KiB chunked streams (5c) |
| Pre-releases | Ed25519 by the release key over a labelled manifest; BLAKE3 per 1 MiB chunk and of the whole package, SHA-256 of the whole package (5f) |
| Recovery copies | X25519 to the recovery key's public half, HKDF-SHA256 salted with the header, XChaCha20-Poly1305 frames numbered by their nonce with the header as associated data; BLAKE3 per file (5g) |

## 7. Threat model summary

Protected: content of messages, calls and files against DHT nodes, network observers and
members acting as bridges; removed members for content and log entries created after their
removal; a group against being taken over by anyone who learns its id (one root per log,
joins checked against the issuing manager's own `Invite`).

Not protected: IP addresses are visible to the DHT nodes a device talks to and to the other
members. Identify tells those DHT nodes, and anyone else that connects, the version most
libp2p peers say (`ipfs/0.1.0`), the library's agent and libp2p's own protocols, not
Fogata's (they go only to peers the device chose to reach and to members and friends that
proved themselves); but anyone that reads a device's announcement on the DHT can connect and
try `/fogata/member/1` itself, which a device must take from peers it does not know yet. A
device keeps its transport identity for as long as it runs, so its announcements in
successive rendezvous windows can be linked to each other (as its public address links them
anyway while it does not move): a new one at a window's turn would close every connection
(calls, transfers, the relay it serves to other members) and leave its members and friends
looking for it on the DHT again. Members acting as bridges or coordinators see who talks to whom and when. A member
can copy what it sees. A compromised device exposes its groups and friends.

Direct chats: only the two friends' devices hold the chat's key, and each checks that every
message is signed by the other. Each keeps what it received, as a member can, and the
attachments it fetched; disappearing messages rely on the other device's application to
delete them. A friend proves membership of the chat's group, but a chat's group asks for no
rosters and gets no relay slots, and this device's relay serves only members of other
groups, so nobody is reached through a chat. A member removed from the last such group it
shares with this device, or left without one when such a group goes here (a group this
device left or was removed from, an invite that expired; a call that ended, five minutes
later, so that its people can go back into it on the same connections, unless the relay
serves them again by then; a call still going on between others through this device then
needs another way), but still here as a friend or through a chat, loses its relay slot here
and its ways through this relay: its connections close (a slot would last an hour, a way
through as long as it is used, for a bridge up to a day or 2 GiB) and it is dialled again a
moment later, as a friend only. One that is neither is disconnected.

At rest: a copy of the data folder (a backup, a synced folder, a stolen disk) does not give
the identity, the friend secrets, the chats or their attachments, which are sealed with a key
the system keychain keeps (5c). It does give the names of groups and members, the group
logs (their keys are sealed to the identity), addresses last reached and the settings. A
program running as the same user (outside any sandbox) can ask the keychain for the key (on Linux and Windows
without the person being asked; for the Flatpak, for the secret the Secret portal keeps for
Fogata, which opens the key files beside the data) and reads what the application has open; plain copies exist
for a while: a video being played, room videos fetched to play, and the copies made while
sealing until the next start (on an SSD removing a file does not wipe it). Losing the key
(a keychain reset, a new computer) loses the folder: there is no other copy of it.

The log of the last days (`diagnostics/`, 5d) is plain on purpose: it is scrubbed as it is
written, and what stays is what a report may carry anyway: the steps, short ids and marks.
Sealing it with the data key would add little to that and lose the lines that matter most:
those written before the key comes from the keychain or when it does not (the errors of a
start that fails), and those a report must read when the core does not run. What remains:
a name not known to the core when a line is written (a stranger asking to join, say, before
the core lists the request) shows in a line only if a library or an error puts it there
outside quotes and outside the lines the core is known to write with names; a file name
with spaces keeps its words before the last one (`beach ‹file›`); a backup of the
folder copies the log as it does the rest.

Updates (5f): a copy installs only what the release key signed, checked in the process with
keys compiled in (and on macOS checked again by the updater, which takes no build older than
or equal to the installed one); on Linux flatpak also installs, by itself, what the Flatpak
signing key signed (below); friends and members only carry releases, and what they say beyond the signed
manifest (that they have a build, which packages) only decides whom to ask. Whoever holds the
release key can make every tester run its code: it stays offline on the publisher's computer;
a leaked one is replaced with the rescue key, kept on a drive apart, for every copy that has
not installed what the leaked key signed.
What remains: a device's friends and members learn which build it runs; one of them can hold
back newer releases from a device that has no other friend or member with them (the release
expires after 180 days, so it cannot hold one back for ever). On Linux the Flatpak signing
key is a second key that can make copies run code: flatpak installs from the loopback origin
whatever commit it signed, newer than the one installed, through `flatpak update`, a
software centre or the portal, with no release manifest and none of its build rules, and
any program of the computer can answer on that port while Fogata does not serve there
(another account, a Flatpak with the network). Without that key such a program can only
keep flatpak from finding an update, see which version is offered, or have flatpak install
a version that key signed and that is newer than the installed one; with it (a key that
leaked), it can install anything on a computer it runs on, and a release cannot take the key
back (a summary it signs only adds keys), nor can the rescue key: Linux copies install again
from `drive/`. So that key is kept as the release key is, in a folder of its own with a
passphrase of its own (5f). The macOS updater and the Windows swap step run
outside any sandbox; pre-release builds are signed with an Apple Development certificate,
not notarized, and not Authenticode-signed.

Friends: whoever uses a friend invite first sends the request, which is why the person
compares fingerprints before accepting. A link or code is meant for one person; a leaked one
lets a stranger ask, not get in. While a request waits, the device it was sent to can tell
when the requester is online; cancelling the request ends that.
