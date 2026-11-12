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

**Contents**

1. [Concepts](#1-concepts)
2. [Repository layout](#2-repository-layout), [the crates and their layers](#21-the-crates-and-their-layers)
3. [Connectivity](#3-connectivity): [rules](#31-rules), [network stack](#32-network-stack-fogata-net),
   [finding each other](#33-finding-each-other), [connecting](#34-connecting),
   [results in the network lab](#35-results-in-the-network-lab),
   [membership proof](#36-membership-proof), [group messages](#37-group-messages),
   [rooms](#38-rooms), [presence](#39-presence), [files](#310-files),
   [direct calls](#311-direct-calls), [network check](#312-network-check),
   [direct messages](#313-direct-messages), [typing indicators](#314-typing-indicators),
   [capability levels](#315-capability-levels)
4. [Voice and video](#4-voice-and-video): [topology](#41-topology),
   [transport and encryption](#42-transport-and-encryption), [calls in rooms](#43-calls-in-rooms),
   [realistic limits](#44-realistic-limits)
5. [Group management](#5-group-management-fogata-state): [the group log](#51-the-group-log),
   [the ladder](#52-the-ladder), [invites](#53-invites), [epochs and keys](#54-epochs-and-keys),
   [MLS](#55-mls-in-progress)
   - 5b. [Profiles and friends](#5b-profiles-and-friends-fogata-friends)
   - 5c. [The application core](#5c-the-application-core-fogata-core)
   - 5d. [The desktop application](#5d-the-desktop-application-app-fogata-ffi)
   - 5e. [Moving an account to another computer](#5e-moving-an-account-to-another-computer-fogata-core-transfer)
   - 5f. [Updates of pre-releases](#5f-updates-of-pre-releases-fogata-update-fogata-net-updates-fogata-core-updates)
   - 5g. [Copies that restore an account without the keychain](#5g-copies-that-restore-an-account-without-the-keychain-fogata-core-recovery)
6. [Cryptography](#6-cryptography-fogata-crypto)
7. [Threat model summary](#7-threat-model-summary)
8. [Testing](#8-testing)
9. [The first milestone (done): the CLI prototype](#9-the-first-milestone-done-the-cli-prototype)

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
                          libp2p-identify, libp2p-stream, turn, spake2), each with a FOGATA.md
                          of its changes.
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

## 3. Connectivity

### 3.1 Rules

1. No servers and no relays run by Fogata or by strangers. No STUN or TURN.
2. Devices only ever forward traffic for **members of a group they share**, and only after
   those members have proved membership on a direct connection.
3. Forwarded traffic stays end-to-end encrypted between the two endpoints. A member that
   forwards sees who talks to whom, when and how much, never the content.
4. The public IPFS DHT is only a phone book: opaque provider records that say "this
   transport id, at these addresses, is looking for this key".

### 3.2 Network stack (fogata-net)

rust-libp2p 0.57, one `Swarm` per device, running every group of the device and its
friends at once. Connections, the DHT and hole punching are shared; each group keeps its own
verified members, key epochs, invites in progress and room. A peer found under one group's
rendezvous key is only asked about that group, so a device never learns of groups it does
not share; a peer whose groups are not known (local network, an explicit address) is asked
about each, by tag only (3.6), so it learns how many groups there are but not which. A device
joining with an invite shows it only to peers found for that group (on the DHT or in the
invite), since the invite names the group and its manager.

| Behaviour | Purpose |
|-----------|---------|
| QUIC + TCP/Noise/Yamux | Transports, IPv4 and IPv6. QUIC preferred (DHT nodes too, see below); TCP for networks that block UDP. Outgoing connections reuse the listening port, which keeps NAT mappings predictable. |
| identify | The far end reports the address it sees us at. Used as a candidate address, never as a fixed "NAT type" label. Fogata's own protocols (`/fogata/...`) are listed only to peers this device chose to reach and to members and friends once they prove themselves (a push tells them then), and, while an account moves, to peers on the local network and those found under the move's code, which look for its protocol; a DHT node or anyone else that connects is not told them. A push goes only when what a connection says changes (addresses said, protocols listed) or the node asks, one at a time per connection, so a burst of changes is a push or two (vendor/libp2p-identify/FOGATA.md). |
| kad | Amino DHT (`/ipfs/kad/1.0.0`), always in client mode: Fogata devices never serve the DHT. The routing table is bootstrapped at the start, on waking up, every 30 minutes while it holds fewer than 20 peers and every 2 hours once it holds more (the library's own refresh is off; it still bootstraps when a peer joins a thin table). Each bootstrap is a minute of lookups through dozens of strangers. |
| mdns | Same-LAN discovery with no internet. |
| ping | Liveness, round-trip time and keeping NAT mappings open. |
| relay (server) | Circuit Relay v2, restricted to verified members of a shared group. |
| relay (client) | Reserve a slot at a member who can be reached. |
| dcutr | Hole punching coordinated through a member's relay slot. |
| stream | Fogata's own stream protocols (files, updates, typing, the media bridge, direct chats, an account's move). Up to 128 streams of one protocol that come in together wait for the task that takes them, which applies its own limits then; libp2p as published keeps one and drops the rest, so several requests at once from one member failed on a busy computer (vendor/libp2p-stream/FOGATA.md). |
| request-response | Membership proof, roster exchange, group messages. A message is at most 4 MiB, and one with more CBOR data items than one per 32 bytes of that is refused before it is decoded: anyone can send one, and tiny items would each take far more memory decoded than on the wire. |

Not used: AutoNAT (it asks strangers to probe us) and libp2p's UPnP behaviour (MPL-2.0
dependency). Port mapping with `rupnp` and `crab_nat` is planned for a later milestone.

**Connections to DHT nodes** (`strangers.rs`). One lookup or announcement on Amino sends
150 to 200 requests to different DHT nodes and takes 15 to 20 s, and a device repeats its
lookups every 30 s while it waits for members or friends (a group's, after three that found
nobody, less and less often, up to every four minutes). With nobody to find (every other
member of the group connected, and not only through another member, whose addresses the punch
slots need and who is lost with that member; for friends, every friend online and no request
or invite waiting) it looks every 15 minutes, until someone goes, something changes or the network
moves. An idle device alone in its group and without friends went from about 1 % of a
core and 143 MB to 0.3 % and 107 MB, 10 to 30 minutes after starting, and opened 2786
connections to DHT nodes in half an hour instead of 6642. It also looks again as soon as its
own first announcement goes through, since those who started at about the same time are
announced by then while the lookups made at the start found nobody: in the network lab the
first connection comes 12 to 20 s after starting instead of about 75. With every connection kept for
60 s after its last use and most DHT nodes reached over TCP, a device held 200 to 600 open
sockets, one file descriptor each; desktop apps on macOS may only open 256. The node
therefore treats DHT nodes as strangers:

- **Nothing is looked up in DNS that need not be.** Many DHT nodes list addresses under
  names of their own (`/dns4/<ip>.<id>.libp2p.direct/.../tls/ws` and the like): every dial of
  one asked the system's DNS server first, about 180 questions in five minutes of an idle
  device, which tell that server (often the internet provider's) which peers the device
  reaches, while the name was chosen by whoever announced it. Addresses the node's transports
  cannot dial (WebSockets, TLS, WebTransport, WebRTC) and every `/dns`, `/dns4` and `/dns6`
  name from DHT records, identify or a friend link are left out, sealed names are only
  opened, and a bootstrap peer's `/dnsaddr` is used only until it says its plain addresses.
  Measured with strace over five minutes: 15 questions, all for the bootstrap peers'
  `/dnsaddr`. A name given to the command line (`--peer`) is still looked up.

- **Candidates from the DHT** (for a group, a friend, a friend link or code, a move): one DHT
  answer adds at most 8 new ones and one lookup keeps at most 16, since a DHT node can name
  any number of made-up members, each with someone else's address. A lookup walks towards
  the key, so nodes on the way answer first and the nodes nearest the key, which keep the
  announcements, last: once a lookup has its 16, a later answer's candidate takes the place
  of the earliest of that lookup's that was never reached (not connected once, not seen on
  the local network, not named by a member) and whose dial is over, so two nodes on the way
  cannot use up the places before the real announcement comes. Made-up ones at addresses
  that answer slowly would hold the places for as long as a dial may take (ten seconds),
  so one that a second answer to the same lookup names (as the nodes nearest the key,
  which all keep the real announcement, do) takes the place of the earliest only one
  answer named even while its dial goes on, which is stopped; nodes on the way would have
  to name a made-up member twice to do the same (netlab `dht-liars`: six nodes on the way
  answering every lookup first with eight made-up members each, at an address that answers
  after twelve seconds, and the two members still find each other at the first lookup after
  they announce). One lookup still takes 64 in all, each
  dialled, however many nodes it asks (it takes eight nodes on the way, answering before
  the nearest ones, to keep a member out). At most 16 never reached are kept for one
  group or friend (or link, code, move) and 64 in all, resting ones not counted; past
  that, a new one takes the place of one never connected first, of those the one that
  failed most, and of those the oldest (one that two answers to a lookup named is the last
  to go among those never connected), unless every one was seen on the local network or
  named by a member, so made-up ones cannot keep a member's new session (a new PeerId at
  every start) out, and a member once connected whose dials fail now is the last to go. A
  friend's found this way is forgotten with it. Until one is seen on the local network,
  its local addresses are not dialled (members' own come sealed, 3.3) and at most 8 of its
  others are.
- **Candidates that never answer** (announcements of sessions that ended, members that
  closed the app) are tried six times, rested for ten minutes, and forgotten after three
  such rests; a later lookup that still finds them announced brings them back. While
  resting they are still dialled once every five slots of the wall clock (50 s): the other
  side may find this device only after it went to rest (a slow DHT on one side), and a
  punch needs both to dial in the same slot, which the shared clock gives them.
- **Idle connections.** A connection closes 10 s after its last request. Members, online
  friends and peers the device is dialling (DHT candidates, invite addresses, friend links,
  relayed paths) are *wanted*: their connections stay open for 60 s after that, and members
  and online friends are wanted again at every slot for as long as they are connected. Any
  other peer that sent or answered a Fogata request (member or friend protocol) is only
  *heard*: its connections stay open as long, but it is not wanted, since anyone can send a
  request, even one that is refused. That includes a device on the LAN, found over mDNS or
  coming in from there, and a candidate once connected, until it proves itself.
- **QUIC for strangers.** Once a QUIC connection to a public address has succeeded in the
  last 2 minutes (so UDP gets through), strangers are dialled at their QUIC addresses only.
  About half of the DHT nodes are known only by TCP addresses (other nodes list just the
  address they saw), and some answer only over TCP; those are dialled over TCP while fewer
  than 32 TCP connections to strangers are open, and skipped otherwise (the lookup asks
  another node at once instead of waiting for a dial that is likely to fail). A stranger
  known only by TCP is also tried over QUIC on the same IP and port at the same time, which
  many of them listen on. A stranger whose QUIC dial failed is dialled over TCP for the
  next 30 minutes, as lookups of the same key keep reaching the same nodes. A stranger's
  LAN addresses are skipped when it has public ones. Before UDP is known to work, and on
  networks that block it, dials are unchanged.
- **Addresses someone else named** are dialled only where this device may send them a
  packet (`addr::OwnNetworks::may_dial`, the one rule every such path takes): never this
  computer (loopback), no address or many (unspecified, multicast, broadcast), an IPv6
  link-local one or IPv4 written as IPv6, nor one of this device's own addresses; a local one
  only on a network this device is on (the same private IPv4 /24, unique-local IPv6 /48, or
  carrier-grade NAT's 100.64/10 while this device has an address there too, a VPN such as
  Tailscale shared by friends), or, where it may be a device at the same place behind another
  router, a private IPv4 one in any /24. Each dial of a local address sends a QUIC handshake
  into this device's own networks (a router's admin page, a VPN peer) at someone else's
  choice. The paths: what the DHT, mDNS and identify supply for a dial, and the sealed local
  names opened for one (checked in `StrangerDials`, also those of an earlier dial kept for
  the next); friend links and invites (the maker chose them: `NodeHandle::dial_invite`); the
  local addresses a member sends (media and lost members only on a network this device is
  on, the punch slots the same place too); mDNS answers (`mdns_address_ok`); and hole
  punches through a member's relay (DCUtR dials every address the other end names: a punch
  that names one this device may not dial is dialled again at its others alone, `punches.rs`).
  Addresses given by hand (`--peer`, tests) and those this device reached a friend or member
  at itself (remembered, lost members, a relay's) are its own choice, and keep their
  loopback ones for copies on one computer. Wanted peers get every other address the DHT
  returned but candidates known only from it, as above.
- **Incoming connections** are bounded: 512 open at once, 32 from one public address (an
  IPv6 /64 counts as one) and 64 from one IPv6 /48, and still in their handshake (closed
  after ten seconds, the connection timeout the node puts around every transport, which
  slow links and each punch's seven-second attempt need, or after five over TCP from an
  address where no member or friend was reached, holding a socket until then, once half
  the places of its kind below are taken: a member on a slow, lossy link losing a packet of
  its handshake needs more, from a new address too; netlab `tcp-slow-trials`, one second
  each way and 10 % loss, 3 of 20 connections were cut before and none now) 8 from one
  public address, 16 from one /48, 32 from
  one block (an IPv4 /16, an IPv6 /32), 32 from one local network address (this computer
  itself not counted), and in all 192 over TCP and 1024 over QUIC. Of those, strangers from
  public addresses get only 128 (TCP: as many as all TCP ones together before, so it still
  takes 16 addresses in 4 blocks to fill) and 768 (QUIC), the local network 48 and 128, and
  the rest is kept for addresses a member or friend is connected from, came from in this
  run (over TCP too, so a member on a network that blocks UDP, never reached over QUIC, gets
  them when it comes back) or was reached at (which the limits per /48 and per block do not
  hold back either) and for this computer
  itself: machines opening silent connections from a few addresses, as
  fast as they are closed, keep no member or friend out on a network that blocks UDP
  (netlab `silent-flood` and `tcp-known`). Nothing a peer says puts an address among the
  known ones (it takes a proof over a connection from it), and one behind a member's
  address gets that address's 8. A member never reached at its address yet, in the same
  block as the flood's addresses, gets no place while the flood fills that block. Open ones from one local network address (an IPv6 /64)
  are bounded too (64; past it, its oldest unwanted one is closed for the new one), and
  from one peer to 8, so a peer the device dials (and so keeps) cannot come in from many
  addresses with all of them. QUIC handshakes take no socket and are not in the TCP count,
  so a TCP flood from many addresses keeps no member out over QUIC; and while 64 or more
  QUIC connections are being set up, one from an address not yet proven to be the sender's
  (QUIC's first packet can carry any source address) is answered with a retry packet
  first, so every one that is counted comes from a real address (`vendor/libp2p-quic`).
  Past 512, the oldest connection of a peer the device does not want (heard peers included)
  is closed for the new one, so a flood from many addresses that keeps asking something
  cheap cannot keep friends and members out (netlab `inbound-flood`: 512 connections from 16
  addresses, and a member behind a symmetric NAT, which can only come in, still does).

QUIC connections share the node's UDP socket, so they cost no file descriptor. Measured on
a home fibre line (Linux CLI node, real Amino DHT, each build on its own):

| | Every connection kept 60 s | Strangers as above |
|---|---|---|
| Open file descriptors over 6 minutes, two groups, no member online (mean / max) | 380 / 633 | 49 / 70 |
| ... of them TCP sockets (mean) | 363 | 32 |
| A second member of a new group finds the first one's announcement (6 measurements) | 37 to 49 s, mean 45 s | 34 to 46 s, mean 40 s |
| One DHT lookup of a group key (mean of about 100) | 18.0 s | 18.2 s |

The node runs in one task. A panic in it (in a library's code, say) loses only the event or
command being handled and is logged. One while handling a request or answer of a peer (or
its typing signal, its connection opening or closing, what it says in identify, its pings, a
hole punch with it, or its relay slot or way through this device's relay) disconnects that
peer; one while handling a DHT answer or a local network announcement, which do not say who
sent them, disconnects nobody, but the lookup the answer was to gets no more answers (so
one lookup makes the node panic once), and the local network address the announcement came
from is not heard for ten minutes. Ten such within ten minutes from at least three peers,
or ten with answers (then something here is wrong, not each of them), make the node start
afresh: every connection closes, which drops all that was kept for each peer, and friends
and members are tried again at the next slot, as after a network change, while the groups,
friends and settings stay and everything holding the node's handle goes on. That ends
every call, so for others' doing it comes at most once in ten minutes, a wait that doubles
each time it comes again soon after, up to four hours. They never stop it, since peers cost
nothing to make and answers a packet each. Three others within a minute make it start
afresh too, and three more within ten
minutes of that stop the node, and the screens are told (`Stopped`); so they are when the
stream of the core's events ends or fails, or a panic loses one of them.
The app then shows a banner
under its header with a "Restart now" button, and starts itself again three seconds later:
it starts a new copy of itself, detached,
with the same arguments and environment (inside an AppImage, the AppImage itself, `$APPIMAGE`,
as for starting with the computer, but only while the executable runs from the AppImage's
mounted folder, `$APPDIR`: a copy started from another AppImage inherits both; on macOS Launch Services opens its bundle again as
a new instance, `NSWorkspace.openApplication` with the `FOGATA_*` variables), and quits as from
the tray, leaving its room. It leaves `window/restart` in the data folder, saying when, so
the new copy waits up to 30 s for the old one to let go of the folder's locks, and knows
when the last restart by itself was: within 10 minutes of one, it does not start again by
itself, and the banner stays until the person restarts it. Debug builds take
`FOGATA_TEST_NODE_PANIC=<seconds>[:<file>]`, which makes the node panic, start afresh and
stop after that long (with a file, only while the file does not exist; the first panic makes it). What the node shares with other tasks behind locks
(members verified, files served) is changed one whole step at a time, so a lock a panic
left poisoned is used as it is rather than read as empty. `vendor/libp2p-mdns` is a patched copy (MIT) of
libp2p-mdns 0.49 (its FOGATA.md lists the changes): its timer for the next expiry of a peer
found on the LAN was a tokio interval with a near-endless period, and polled more than 5 ms
late (a busy computer) adding that period to the time overflowed and panicked. Its period is
a year now.

What the node tells the application waits in a bounded queue, and only notes and typing
signals, worth nothing late, are ever dropped: past 1024 events they give way first. Events
that only say how something is now (a member's presence, a friend's status, a group or the
friends changed, a profile, an animation, a capability level, a group a friend offered, a
friend request) replace the same one still waiting, so there is at most one of each per
device, group or friend list, and they are never dropped (an offer is taken only while the
queue is not full, a request needs a link or a code of this device's): a member saying it
is away and back 20 000 times while the application was busy used to push a group change out
of the queue. One that carries what it says goes last, where the newer one was sent, so that
it still comes after what came between (a friend gone and back keeps its newer status); a
group or the friends changed keeps its earlier place, since the application reads them anew
then. Those count towards the bounds below all the same, in number and in bytes (an animation
is up to 550 KiB): 128 animations once held 72 MB without the queue saying it was full.
Past 16 384 events waiting, the node takes no peer request that would add one (a
group or direct message, a call step, a group offered) and refuses it, so that the
peer sends it again or this device asks for it later (room events by their ids); nor does it
ask for or open room events. Once it is not behind, the room of each group something was
refused in is asked for whole again of one member, three groups a heartbeat at most, groups
and members in random order, and of another member only when the one asked did not give it:
asking every member of every group at once brought the same room back from each and could
fill the queue again. The same past 64 MiB of them, as near as their sizes are known
(messages, signals, logs and pictures they carry), and while the core says the screens are
behind (below). A call signal is never refused: its sender would not send it again and the
call would wait for it; signals are few and small (8 a second, 64 KiB, from participants of
this device's room). What still comes (connections, answers to
its own asks, its timers) is bounded by what it is about. 65 536 events or 256 MiB are last
bounds for a fault elsewhere: past them an event is dropped, and one `Resync` goes after the
last one dropped, on which the core reads the groups, the members connected (direct chats
too, each read as newly connected, so a friend that went and came back unseen is sent what waits; a chat told connected twice otherwise sends it once), the friends and the voice rooms
(when each was last open) anew from the node and the screens look again, the rooms
included. A step of a call with a
friend and a taking out of a room, which reading anew does not bring back, have 1024 events
and 16 MiB more. A call signal dropped there is not brought back either: one `SignalsLost`
for its sender goes after the last of its signals kept, and none of its signals is kept
after it until the application received it.

The core's events for the screens (`CoreEvents`, crates/fogata-core/src/events.rs) are
bounded the same way, so that screens that stop reading for a while (a window stuck) leave
neither the core nor the node holding more and more. Events that say something changed or how
something is now (the groups, friends, calls or asks to join changed, a member connected or
gone, a friend online, a chat changed, someone typing, a download's progress, away) replace
the same one waiting, so 20 000 changes made here meanwhile (away and back, a group renamed)
leave a handful. Notes give way past 256. Past 1024 other events (or 16 MiB) the core tells
the node the screens are behind, and the node holds back as above; past 4096 (or 64 MiB), for
a fault elsewhere, an event is dropped and the screens are told the groups, friends, calls and
asks changed, which has them read all that anew; a call ringing, answered or ended, a group
joined or left and a taking out of a room have 256 more. Call signals give way past 64 of one
sender waiting, or 8 MiB of all, with one `SignalsLost` after the last kept, as in the node.
Between the core and Flutter the bridge gives the screens at most 256 events they have not
said they took (`events_taken`, after each one is handled); the rest wait in the core's queue.
On `SignalsLost` the screens let go of the connection with that participant and make it anew,
as with one that just came in: the side with the smaller id offers again, and the other asks
for an offer until one comes (what the two agreed may be half done on either side, an offer
or an answer missing).

### 3.3 Finding each other

Discovery is layered; every layer runs in parallel and none is required.

1. **Local cache**: the addresses where friends and members were last reached (direct,
   hole-punched or local QUIC connections; up to 32, for a month, in `meta.json`, each with
   the device reached there), dialled at start and whenever the network changes, before the
   DHT finds anyone: only those of devices still friends or members, and a local address
   only while this device is on a network of the same block (a private IPv4 /24, a
   unique-local IPv6 /48, or carrier-grade NAT's 100.64/10, which VPNs such as Tailscale
   use all of), since elsewhere it names some other computer. A peer reached there proves itself as usual; one that may be a friend is
   probed.
2. **Invite links**: addresses of members who were online when the invite was made, and a
   rendezvous of the invite's own where the managers wait for the invited device (5.3).
3. **mDNS** on the local network, asked every 30 seconds (libp2p asks only every five
   minutes once anyone answered, so a lost answer or a friend that restarts waited that long).
   Queries and answers go from the socket on port 5353 of each interface, as RFC 6762 has
   responders do, so every copy on a computer is one sender to the others (macOS's firewall
   checks each new sender a socket hears from). Anyone there can answer with made-up peers,
   so only answers to this device's own queries of the last 30 seconds (one query interval:
   an answer the firewall held a while still counts), from its own network, are taken (the query id is 16 bits sent in
   the clear, so that keeps out the internet and unasked floods, not a neighbour that
   answers the queries: the bounds that follow do); what they tell is kept for at most a
   minute unless heard again; at most 128 peers are kept, 16 told of by one address (a new
   one takes the place of the one of that address that expires first; a peer stays with
   the address that first named it until a connection is made at an address another one
   told of, which then has it, and another address adds it only addresses of its own, in
   the place of one never reached); and only 8 new peers told of by one address (the
   response's, not those it names, which are whatever its sender likes; a peer counts for
   the first response that lets it in, what looks like its own answer, sent from an
   address it names, first), 32 in all, are dialled at once, then 8 and 32 a minute. An
   address where a member or friend was reached has its own 8 and counts nowhere else (a
   member started again comes back under a new transport identity, so only its address can
   be known beforehand), and this computer's own addresses have 64 at once and 64 a minute
   of their own (copies here: tests, the call lab, a second profile; a neighbour that sends
   from this computer's address, which Linux drops but macOS and Windows may not, gets no
   more). A peer that proved membership here or is a friend recognised in this session is
   no new peer and takes no share. The others wait, the answers of this computer and of
   known addresses first, each in the order they were told
   of, at most 128 (as many as mDNS keeps; one it no longer keeps leaves), and are dialled
   as those shares fill again: mDNS tells of a peer once, and one that keeps answering is
   never told of again, so a member turned away while a crowd of devices on the network, or
   a neighbour making up peers from a few addresses, had spent the shares was never
   dialled. The behaviour hands on what its interfaces told at most 16 addresses at a time,
   so that a peer told of is still known when the node asks who told of it (copies on one
   computer past its 16 would otherwise push a new one out in the same go). A peer is dialled once, at the addresses every
   response gave, each response's first before any one's second, so that a neighbour's
   dial at made-up addresses cannot hold the swarm from dialling the real one. Responses
   that come in a later event, while that dial is still going, are dialled at once beside
   it, at the addresses not dialled yet (mDNS tells of each only once): at most 16 of a
   peer's at once, and what comes past that is dialled when those dials have all failed.
   A peer whose dials all failed is dialled again twice, 2 s after each failure
   (`MDNS_RETRIES`), at the addresses its own answers gave (sent from them), each try on
   that answer's share of new peers (waiting for it when it is spent) and never for an
   address not heard now nor one mDNS no longer keeps: a first
   attempt lost while macOS's firewall checked the new sender would otherwise wait a
   minute, until mDNS told of it again, and a neighbour cannot make this device dial
   anyone else more than once. The tasks
   that ask and answer on each interface stop with their node (they went on answering with
   its dead addresses for up to a query interval). A
   peer found there is asked who it is, but its connections are not kept wanted until it
   proves something (netlab `mdns-flood`: a neighbour answering every query with 80 made-up
   peers and sending 50 unasked answers a second made a device take 27 108 addresses, grow
   to 618 MiB and spend 138 s of processor time in 90 s, and miss its member's message;
   now 242 addresses, 57 MiB, 5 s, and the message arrives).
4. **Amino DHT**, with the public bootstrap peers used only when the cache is empty.

When a friend or member goes without a goodbye (to another network, say), the device looks
it up at every 10-second slot for two minutes rather than at its usual pace, since it has to
learn the new address to punch through to it. A device whose public address changes without
any local change (a router or a carrier giving it a new one) announces itself again as soon
as two peers confirm the new address, and tries friends and members again as after a network
change; network changes that come within two seconds of each other count as one, and a slot
at a member's relay is no network change. In the network lab (`cone-cone-move`) the other
side has the moved device again about 110 seconds after the move, 35 of them for the old
connection to time out; before, it did not find it again. An announcement the DHT does not
take in time (common on a slow or lossy uplink right after starting) is tried again after 15
seconds, then 30, 60 and so on up to four minutes, instead of at the next refresh (up to half
an hour away): until it is taken, a device that cannot be reached directly is never found,
and the other side never opens its own router for it. While a friend link or code is shown
or used here it is announced again every minute: an announcement reaches the nodes closest
to the key as this device sees them, on a slow uplink only a few of them take it, and the
other side's lookups missed it for more than ten minutes between a PC and a Mac on a
satellite link.

**Rendezvous key.** Members meet under a DHT key that only members can compute and that
changes every epoch:

```
epoch          = floor(unix_time / 6 h)
rendezvous(e)  = SHA-256 multihash of HKDF(group_secret, "fogata/v1/rendezvous" || e)
```

A member announces itself as a provider of the current key (and queries the current and
previous one). The provider record carries the transport `PeerId` and the addresses the
device has observed for itself (seen identically by peers on different networks, /16 for
IPv4 and /48 for IPv6, so one machine posing as several peers, or a few at one provider,
cannot choose it: two networks with a member or friend among them, three of strangers
otherwise, all of them strangers this device dialled itself (the bootstrap nodes, the DHT
nodes its own lookups lead to), since anyone can connect from three cheap machines at one
cloud provider and say anything, and a stranger that came in by itself counts for nothing
even beside one this device dialled; the same for the address announced when the device's
network changes the port for each destination). The DHT nodes a device dials as it starts
are on many networks, and tell it its address within seconds (every confirmation logged in
the lab and on the internet had all its reporters dialled). The log says when an address is
taken (`this device's public address confirmed`, with how many peers, networks and dialled
ones), and when strangers that came in agree on one not taken. Only an address a member or friend
reported too counts as this device's own when a candidate behind it is taken for one of
its old sessions (rested at once, not punched): strangers on two networks could otherwise
make it give up on another member.

The group secret here is that of a key epoch (5.4). A member announces under the two newest
it holds, so members who missed one key change still find it, and looks up two more before
those, each for two weeks after the key that replaced it began: a member that missed several
key changes announces only under old ones, and is found there and sent the log. Nothing is
announced under those older ones, so a removed device that knows them does not see the
members' announcements; one that announces there during those two weeks learns the address
of whoever dials it. A key whose successor says it began more than a day after this device's
clock is not looked up at all: that time is its author's to choose, and one near the end of
all time would keep it looked up for good.

Private LAN addresses are never published in the clear: public DHT nodes drop them anyway,
and they are nobody else's business. A friend or member at the same place but behind another
router (two routers at home, one public address that does not lead back inside) still needs
them, so they travel sealed inside a name, `/dns4/<hex>.fogata/udp/<port>/quic-v1`, that the
public DHT keeps. There is one name for friends, sealed under a key from the device's friend
rendezvous secret, and one per group (at most six names in all), under a key from the group's
current rendezvous secret. An announcement always carries six: the ones not needed (and all
six when the device has no local address) are random bytes as long as a real one, which
open to nothing, so DHT nodes cannot count a device's groups. A device with a profile adds a
seventh, longer name that proves to its friends the announcement is its own (5b); it does
so with no friends as well, since the profile holds its friend secret from the start, so the
seventh name does not tell who has friends. Each holds up to three local IPv4 addresses, encrypted and
authenticated with an 8-byte nonce and an 8-byte tag (`seal_short`, HMAC-SHA256), so it fits
in one DNS label. The names go only into DHT announcements, never into identify or to other
peers. DHT nodes see random hex; a device that knows one of the
secrets opens the name when it dials that peer, and a name is never looked up in DNS. The
port is not sealed, so a DHT node that keeps an announcement could change it to point dials
at another service of a local address: a name opens only with a port of the devices' range
below (a command-line device started with `--port` outside it is not reached this way).

Each device listens on one port of its own, picked at random between 20000 and 39999 the
first time and kept (another one in that range if it is taken), so a firewall rule for it
keeps working. The device on the main router of a two-router home is only reached by coming
in, so when this computer's own firewall keeps Fogata out, the app shows the one command that
lets it in (`fogata-core` `firewall.rs`; anything it cannot read counts as nothing in the way):
- Linux with ufw letting nothing in unasked: the rules are read without privileges, and the
  command (`sudo ufw allow <port>`) lets that port in.
- Windows: "Cancel" on the firewall's question the first time Fogata listens leaves active
  inbound rules that block the program, and a blocking rule wins over any allowing one. The
  local rules are read with `reg query` from the registry, which shows them the same in every
  language (`netsh` translates its output); only a rule blocking UDP into Fogata's program from
  anywhere counts. When such rules block some kinds of network (private, public, domain) and
  not others, PowerShell's `Get-NetConnectionProfile` tells which kind this computer is on
  (the networks that reach the internet). The command, for PowerShell as administrator, turns
  those rules into allowing ones (`Set-NetFirewallRule -Action Allow`). Having no rule at all
  is not reported: Windows then asks the person itself, and rules for the port or for every
  program may let Fogata in.
- macOS: the application firewall's own tool (`socketfilterfw --getglobalstate`,
  `--getblockall`, `--getappblocked <Fogata.app>`) tells whether it blocks every incoming
  connection or Fogata, and the command (`sudo socketfilterfw --setblockall off` or
  `--unblockapp`) undoes that. The sandboxed release build may not be allowed to run the tool;
  it then shows nothing.

Off Linux these looks start other programs, so they run off the async threads, and the watch
that tries friends again once the firewall lets Fogata in ends as soon as nothing is in the
way.

**Privacy notes.**
- Provider records live 48 hours on DHT nodes, so rotating the key does not erase older
  announcements; it only stops new ones.
- The transport `PeerId` is not the device identity, and a new one is made each time the
  app starts; a fixed one would let anyone link the announcements of every run. It does not
  change while the app runs: announcements under the epochs of one run, and the networks a
  laptop moves between during it, can be linked by that `PeerId`. Changing it within a run
  would drop every connection and call, and is not done.
- Relayed addresses (through a member's relay slot) are never published or shared with
  DHT nodes: they would reveal which member a device is connected to. Members find relayed
  paths by asking each other for rosters instead.
- DHT nodes still see the IP address of whoever announces or asks. Fogata is not an
  anonymity tool.
- The names in one announcement do not count a device's groups, but the keys it announces
  under do, to whoever sees them together: one or two per group (5.4), one for friends and
  one for each friend link, code or request in progress, all from the same `PeerId`. DHT
  nodes see only the keys close to them, so this takes many nodes or watching the device's
  own traffic.

### 3.4 Connecting

| Situation | Method |
|-----------|--------|
| Same LAN | mDNS, direct. |
| One side reachable (public address or mapped port) | Direct dial. |
| Both behind NAT, no member online to help | **Scheduled hole punch.** Both devices find each other in the DHT and try at the same wall-clock slots (every 10 s, each attempt shorter than a slot). In each slot one end dials and the other dials as listener, which makes QUIC send packets that open its own NAT; the roles swap every slot, so a device that cannot be reached still gets turns to dial out. No coordinator, no relay. Works when both NATs keep the same public port for every destination. |
| Both behind NAT, a member C is connected to both | **Member as coordinator.** One side reserves a slot at C; the other connects through C; DCUtR synchronises a punch. C forwards a few kilobytes. |
| Punching fails | **Member as bridge.** The connection through C stays up. C forwards encrypted bytes (Noise/TLS between the endpoints runs inside the circuit). Can be turned off per device and is bandwidth capped. |
| Inside a group, someone reaches nobody directly | Messages are passed on through members to those their author is not connected to (3.7). |

Once two devices have a direct connection, they keep it open with heartbeats every 20 s (see
"Connections to DHT nodes" in 3.2), so it survives the coordinator leaving.

A direct way can also stop working while both devices still reach the other members: a
Wi-Fi that starts isolating its clients, a VLAN, a NAT that drops its mapping. Each device
checks every 2 s when its QUIC connections to each member last received a packet; QUIC
keep-alives every 5 s make a live connection hear something at least that often. After
10 s of silence it asks the other members for their rosters at once and connects to the
member through one that still reaches it, beside the silent connections; as soon as that
way is open, the silent connections are closed, so messages, files and the call's media take
the way that works and the member never shows as gone. A member whose last connection
closes (over TCP, which only the pings notice) is looked for through the others at once the
same way.

Two members that count each other in ask each other for their rosters at once, each as soon
as it has counted the other in; the one that asked first may be refused, not counted in yet.
It asks again (with the log heads, so a sync starts if needed, and for the rooms) as soon as
the other asks for its own roster, which says it is counted in now. Without that, a member
that came in through one other member, say someone rung into a call by a friend, learnt of
the rest only at the next heartbeat, 20 s on, and anyone who left meanwhile it never heard
from. A member reached only through another member is still dialled at its public and
local addresses every fifth punch slot (50 s) (a private IPv4 one in any /24, which may be
another network of the same home; a carrier-grade NAT or unique-local IPv6 one only of a
network this device is on), so the direct way comes back once it opens
again; the way through the member closes a few seconds later. In the lab
(`member-lost-direct`) both ends talk through the third member 8 to 10 s after the cut, and
are back on a direct connection within a minute of the way opening. Three copies of the app
on one local network, in a call, with nothing passing between two of them for 90 s: their
voice stopped for 11 to 13 s and then went on through the third, and went back to the local
network 26 s after the way opened, without a gap.

A member on the local network whose last QUIC connection closes while it is still a member
(an outage longer than QUIC's 30-second idle timeout, or pings that go unanswered) is dialled
again where it was reached there and at the local addresses it sent: 2 s later, then twice as
long each time up to every 16 s, for five minutes or until a QUIC connection is back. That
holds too while a TCP connection outlives the outage, which carries no media and comes back
only once its own retries do. Up to 32 members at once; a local address only while this
device is on a network of the same block (as for the local cache above); failures of these dials never rest a member as a stale
announcement; public addresses are left to the punch slots, which a dial on its way would hold
off. Only that session of the device can answer: the dial names its transport identity. On one
local network nothing else brought it back: mDNS tells of a device again only once it has
forgotten it, which answers every 30 s never let happen, and the punch slots dial no local
address. Two copies of the app in a call on one local network, with one's link cut for 45 s:
the member was back 2 s after the link returned and the voice 20 to 21 s after; for 120 s, 10
to 13 s and 18 to 31 s (the rest is the call setting itself up again). Before, neither came
back in ten minutes. Outages under 30 s keep the connection and the call
where they were.

Every device keeps a relay slot at up to eight directly connected members, so that a member
who reaches only one of them can come through. Every device serves those slots whether or
not it can be reached from the internet: a member only has to be connected to both ends, and
libp2p's default (serve only once an address is confirmed from outside) left members on a
network without internet, or behind many NATs, unable to carry anyone. The app names the
member that carries someone ("through Ana") to the groups that member is in, and only to
those: a group where it is not a member hears "through a member", as before the member
proves membership. How a member is reached is told when it changes, a better way at once and
any other once it has lasted two seconds: two devices with several ways between them (local
IPv4, global IPv6, TCP) open and close connections at once, which would make the way shown
flicker. (With three slots, a device connected to several members on its own network could
spend them all there; in a six-person test call, a device behind NAT reached one member but
none of the four others on a second computer.)

Known limits:
- Two NATs that pick a random port for every destination cannot be punched in useful time
  (published NAT traversal measurements: 0.01 % after 20 s; 170,000 probes per side for
  99.9 %). Those two
  devices need a member as bridge. With no member online, they cannot talk at that moment,
  and the app says so.
- rust-libp2p 0.57 disables QUIC connection migration (and datagrams, which Fogata's copy of
  libp2p-quic turns on for media, 4.2). A network change (Wi-Fi to mobile) means reconnecting
  and resuming, not a seamless move.
- Networks that block UDP leave TCP, where punching is less reliable.
- A device behind a NAT that changes the port per destination cannot publish a usable
  address. It can still find members and dial out; members reach it through a member it is
  connected to.

### 3.5 Results in the network lab

`tools/netlab/netlab.sh` builds the scenarios below in unprivileged network namespaces, with
three lab DHT nodes standing in for Amino, in three networks (203.0.113.0/24, 198.51.100.0/24
and 192.0.2.0/24: strangers on three networks make an address taken). "Cone" is Linux masquerade (keeps the source port,
filters by address and port); "symmetric" is masquerade with a random port per destination.
The lab DHT nodes write down every announcement they take, as a stranger running a DHT node
would see it. Hidden commands of the command-line client exist for the lab only (`lab-liar`
tells every peer it meets a false address of its own, `lab-open` opens the sealed names in
those records with one device's secrets, and others attack or probe a device on purpose),
together with `update run --lab-unchecked` and the other `--lab-*` flags, `release --lab` and
`media-bridge --lab-loss`: all of them are behind the program's `lab` feature, which the
netlab builds with and the program that ships is built without (a test checks that the
default program does not know any of them).

| Scenario | Result |
|----------|--------|
| cone + cone | Connected by the scheduled punch, no member involved. |
| cone + cone + public member | All connected directly or punched. |
| cone + symmetric | No contact (known limit). |
| cone + symmetric + public member | Both reach the member; they talk through it. |
| symmetric + symmetric + public member | Both reach the member; DCUtR through it fails as expected; the member bridges. |
| cone + cone + public member, then nothing passes between the two cone devices for 90 s | Both talk through the member 8 to 10 s after the cut, without showing the other as gone; back to a punched connection within a minute of the way opening again. |
| two routers at home, one public address without hairpin, no firewall on the devices | The device behind the inner router opens the other's sealed name and connects to its local address. |
| the same, the outer device's firewall letting its port in | Connected the same way. |
| the same, the outer device's firewall letting nothing in | No contact (known limit; the app shows the firewall command). |
| a public device told false addresses of its own: by three peers behind one public address (two agreeing), by two more at addresses of their own in one /16, agreeing, and by three on three networks that connect by themselves, agreeing (`fake-observed`) | It takes and announces only its real address, which the three DHT nodes it dialled report from three networks: strangers' reports count from three different networks (/16, /48), and only those of strangers it dialled. |
| a public device flooded with 512 incoming connections from 16 addresses (64 peers, as one peer gets 8 in) that ask it something cheap every half minute, and a member behind a symmetric NAT (`inbound-flood`) | The member comes in, one of the stranger's connections closing for it, and its message arrives. |
| a public device sent silent TCP connections from 8 addresses, and a device behind a router sent them from one local address, each as fast as they close, each device limited to 2048 open files (`silent-flood`) | Each holds at most a few dozen sockets more than usual (the local one used up all 2048, and could no longer write its files, when the local network had no limit), and its member's message arrives. |
| two members on one local network, mDNS only, and a neighbour answering every query with made-up peers and sending unasked answers (`mdns-flood`) | They find each other and talk; a bounded number of the made-up peers is taken, and few are dialled. |
| two devices with local addresses, friends and members of a group (`lan-sealed`) | Their announcements on the lab DHT hold no local address in clear, only sealed names; the other device opens them with the friend secret and with the group's, a stranger with secrets of its own opens none. |

The lab routers drop unsolicited packets addressed to themselves, as home routers do. If a
Linux router accepts them instead, the connection-tracking entry left by an early packet from
the other side makes it pick a different public port for the device, and the punch fails.

Later improvements, to be decided with measurements: port mapping (UPnP, NAT-PMP, PCP),
port-guessing for one hard NAT against an easy one, routes through several members
(A → C → D → B), and mutual friends (not only shared group members) as coordinators.

### 3.6 Membership proof

Every new connection starts with a mutual challenge before any group traffic:

1. Each side sends a random 32-byte challenge and, instead of the group id, one tag per key
   epoch it knows (at most four, newest first): `HMAC(auth_key, "fogata/v1/which" ||
   group_id || epoch || asker PeerId || answerer PeerId || challenge)`, cut to 16 bytes. Only
   holders of that epoch's key recognise the group in it, and a fresh challenge makes every
   tag new, so tags cannot be linked to a group or to each other.
2. The other answers with the newest of those epochs it also knows, its device public key, an
   HMAC under that epoch's `auth_key` and an Ed25519 signature by its device key, both over
   `"fogata/v1/hello" || group_id || epoch || prover PeerId || verifier PeerId || challenge ||
   device key`. The device key, the HMAC and the signature travel sealed
   (XChaCha20-Poly1305 under a key derived from the epoch's `auth_key` and the challenge,
   bound to the group, the epoch, both `PeerId`s and the challenge), so only holders of
   that epoch's key can read the answer. A device asked about a group or an epoch it does
   not have answers with random bytes of the same shape and asks nothing back: whoever
   learnt a group id (someone who saw an invite, a peer on the same network) gets neither
   a device key nor a yes or no. The tags are tried against each group's four newest
   epochs first; trying them against every older one (for a member that missed several key
   changes) is what a challenge naming no group costs, so a peer that is not a member yet
   gets that only four times at once and then once every 15 seconds, and all such peers
   together 64 at once and then four a second, of which strangers at one address (an IPv6
   /64), whatever peers they come as, get 8 at once and then one every 15 seconds: it takes
   some sixty addresses to drain that shared bucket. A member coming back after several key
   changes is recognised only by such a search, so a peer vouched for by something a stranger on the internet cannot
   bring (it comes from an address a member or friend is connected from or was reached at,
   or from the local network, or through a member's relay) takes from a bucket of its
   voucher's instead: 16 at once and then one a second for each such address (an IPv6 /64),
   each address on the local network, and each peer through a relay (one sharing them would
   let a neighbour, or anyone the relay serves, use them up for a member coming back that
   way), bounded to 1024 vouchers kept, of which 64 on the local network and 32 through one
   relay. Peers that are neither members nor friends
   yet get 256 answers at once from one address (an IPv6 /64 counts as one), or through
   one member's relay (every peer through it together), of either
   protocol, and 32 more each second; past that a challenge gets the random answer, and
   anything else is refused. In a release build a challenge costs 3 µs with 50 groups of
   one key epoch and 15 µs with 50 of 300 (a search of every key: 5 µs and 164 µs).
3. The device key must be in the signed member list (section 5) as the verifier knows it,
   given a key there by a manager since it came in: added by a manager, or let in by one
   with a `Welcome` signed on its `Join`. Any member can write a `Join` with a ticket it saw
   in the log, so a `Join` by itself lets nobody in (5.1).

A key that someone outside the group may hold (an older epoch, or the current one while a
device that left still has it) proves nothing about who asks. Asked with such a key, a
device answers with random bytes as above unless the asker has already proved itself a
member, challenges the asker instead, and once it has proved itself asks it to ask again. A
member that missed a rotation therefore still connects and catches up, while a removed
device that kept an old key never reads a proof. An older epoch whose key every device given
it still holds as a member counts like the current one. A member left without the current
key (the epoch started with a box for it that does not open, or none) answers with the
newest key it has, as a member that had not seen that epoch yet would, so members in that
state still reach each other and get the new key a manager makes (5.1).

Both `PeerId`s are authenticated by the transport handshake, so a proof cannot be replayed on
another connection. Only peers that passed may use the member relay. Both ends challenge each
other at once, so one may get the other's profile and presence before it has the proof:
they are kept until then, only from a peer this device challenged, only a profile that
decodes and is signed (a picture's animation only if the same device signed it), 4 MiB in all
for at most 64 peers, 4 of them from one address and 8 through one member's relay, and
dropped once the challenge fails or times out. After the proof both
sides exchange the log entries the other lacks, 256 KiB at most in one answer and in the
entries sent back that the answer asked for, so that each arrives within a request's 20
seconds also over a slow link (2 s at 1 Mbit/s, 16 s at 128 kbit/s); while an answer brings
new entries the next is asked for at once, and the rest follows at the next roster, which
starts another exchange while the logs differ.
What a member asks this device for (the entries it lacks, a roster, a room's past whole or
by event) is built on the node's one task, and a room's past alone fills a 1 MiB answer, so
each member device has a budget of what it is sent in answers to those asks: 32 MiB at once,
enough for a member back after a long time away, then 256 KiB a second, each answer counting
at least 4 KiB (with a roster, the list of entries this device sends back to sync with it).
Kept per device across connections; an ask past it is refused and made again later.

Every entry a device takes costs it a signature check on the node's one task, and every entry
it keeps is kept for good, so what one member (or one friend offering a group) may make it
check is budgeted, whatever request carries the entries (`Push`, a sync's answer, a group offered): 4096 entries and 4 MiB at once, then one entry and 4 KiB a second, kept per
device across connections. Entries past that, the last of a message, are left out and come
with a later sync; a group offered whole that does not fit is refused and offered again. A
member sending message after message of entries that do not verify held a node for about 1.5
s per message; it now holds it for at most about 0.2 s once (measured in release on a 16-core
PC). A device that only knows a group's id gets its entries looked at only when one of them
is this device's removal written by a member. A device that was removed while away is sent,
once, the signed log up to the entry that removed it, so it learns why it is refused and
nothing that happened after. It takes that from a member it cannot verify only because the
entries remove it. A member removed while connected gets the entries that remove it before
its connection is let go, so it learns it at once.

### 3.7 Group messages

```
header     = { version, group_id, sender device key, timestamp, random message id }
ciphertext = XChaCha20-Poly1305(message_key, random nonce, plaintext, aad = header)
signature  = Ed25519(device key, "fogata/v1/group-message\0" || header || nonce || ciphertext)
```

Receivers check the signature and decrypt; a forwarded copy cannot be altered or forged. The
label that starts what is signed keeps a message's signature from passing for one of anything
else the device key signs (profiles, log entries, proofs), and theirs for a message's; the
header's version (3) says so, and messages of version 2, signed without it, are refused.
Nothing keeps sealed messages on the disk (a direct chat keeps what they held), so none
written before is lost.
Message ids remove duplicates when a message comes by several members; an id is
remembered only once its message opened and verified, so a damaged copy that arrives first
does not stop the real one.

A sealed message over 192 KiB is dropped unopened (the largest a device writes, a file shared
with a video's first picture, is under 100 KiB). Each connection may hand a device 1024
messages and 16 MiB at once, then 128 messages and 2 MiB a second (it passes on others'
messages too); past that, messages are refused unopened. Each member's own messages, counted
once they open, may come 256 and 8 MiB at once (a room joined with dozens of files shared),
then 32 and 512 KiB a second; past that, they are neither shown nor passed on, so one member
cannot have the others carry a flood to everyone. Events asked for again (a room's past) are
not counted.

A group message goes once to each member, not to everyone from everyone:

- The author sends it to every member it is connected to as `Spread`, which names them (the
  last 8 bytes of each transport peer id, at most 64). A member that gets a new one passes
  it on only to the members it is connected to that were not named. Of those, it leaves out
  the ones listed in the roster (the members it is directly connected to, asked at every
  heartbeat and trusted for 45 s) of a named member that comes before it in an order drawn
  for each message from its id, since that one passes it on; with no roster to go by, it
  passes it on itself, so a message goes to some members twice rather than to none. A
  member whose connection went silent is not named. In a group where everyone is connected
  to everyone, a line in the room is N - 1 transfers instead of about N². Measured with
  command-line members on one lab network, for each line, all members together: 6 members
  22 KB sent and 12 ms of CPU before, 4.9 KB and 3.9 ms now; 20 members 288 KB and 121 ms
  before, 26 KB and 14 ms now. Presences (every 20 s) went the same way: a quiet room of 20
  sent 365 KiB/s in all before, 74 KiB/s now.
- Devices before this version take only `Deliver`, which they flood to every member they are
  connected to. Only devices that advertise `/fogata/spread/1` in identify (a protocol that
  carries nothing) get `Spread` and `RoomEvents`; the others get `Deliver` as before. A new
  message from an older device is passed on to the members that device's roster does not
  list, as `Spread` naming them all.
- Every `Here` names the ids of the room's latest events its sender has (joins, leaves,
  chat, files; at most 16, from the last 2 minutes). A member that lacks some asks the
  sender for them (`RoomEvents`), or the member that passed the `Here` on, at most every
  10 s per event; one that lacks all 16 asks for the whole room (at most every minute). So
  an event lost on the way (a request that failed with a connection that went away, a stale
  roster) arrives within a `Here` or two, about 20 s at most. Of those 16 a device keeps
  whole only the newest that fit 1 MiB together (the newest always), the size of one answer:
  the others it still names, but no longer gives.

Every message and request to a member is a request of the member protocol, on a stream of its
own. A connection takes 16 such streams at once, both ways together, and drops any stream
past that with what it carried, so each device has at most 6 requests on their way to one
member; the rest wait their turn, in order (at most 1024, the oldest going first). A burst
of messages (lines pasted at once, many files shared at once) arrives whole. A request that
goes unsent, past that bound or waiting when the member's last connection closes, is let go
as one lost on the way: a presence goes again at the next heartbeat, a challenge, an invite
or an ask for the rooms can be made again, and a direct chat's requests after it wait to be
sent again in order. New log entries
pushed to every member are held once, whatever the number of members and of requests
waiting for them (`SharedBytes`).

### 3.8 Rooms

Everything said in a group is said in one of its voice rooms. A group has named voice rooms
(5.1; a first one always), each with at most one room at a time, and a room exists only in the
memory of the members online (`fogata-state::room`, no I/O). Voice rooms are apart: each has
its own people, chat, files and calls, so those in one never connect with those in another,
and a device is in one room at a time. Every room event names its voice room (left out for
the first one, which is how older devices see every room); a member keeps one room tracker
per voice room. Joining a voice room that already holds the group's most people per room is
refused by the joining device; someone who seems away (below) keeps no place, so a device
that crashed does not hold one until it is dropped. The app shows each voice room's people
against that most, as "3/8", wherever it lists voice rooms (the group's column, the live
cards on home, the group settings) and in the room's header, and says "Full" in amber when it
is reached; the most comes with the group's log (`SetLimits`), so every member shows a change
as it arrives.

- Room events are group messages (3.7), spread to every member: `Join` (with
  the room's random 16-byte id and start time), `Here` (every 20 seconds), `Leave` and
  `Chat`. Members outside the room see who is in it and get its chat too, so a member that
  comes online can be told the room by anyone.
- A member that comes online, or connects to another member, asks it for the room: the latest
  join of each participant, the files shared and the chat (up to 500 lines), as the sealed
  messages that were sent. They come in parts of about 1 MiB, who is in the room first: long
  chats and many videos hold more than one answer takes; parts are asked for up to event
  8192, a little over what a group's rooms can hold. Chat is ordered by sender time and
  message id.
- `Here` counts as a join for members that missed one, so a participant that started alone
  and connected later is seen within 20 seconds; it is also sent at once to every member that
  connects.
- Chat or a file from someone not known to be in the room, or any event of an unknown room,
  makes the member ask for the room's events again; copies among them are applied even if
  seen before, since the first copy may have come too early to place.
- A participant that takes another name says it in the room at once (a `Here` under the new
  name), so members it has no connection to, which never get its profile, show it too.
- A participant that leaves on purpose sends `Leave` (also on Ctrl+C in the CLI). One that is
  not heard from for 45 seconds shows as away and after 2 minutes is dropped; its events
  from before, which any member could send again, no longer bring it back. One this device
  has no way left to hear from (no connection to it, and every member it is connected to in
  the group says in a fresh roster that it reaches nobody else) is dropped once not heard
  from for 25 s, a little more than a `Here`: a leave it said could not reach this device
  either, as when the member that passed its words on left first, or it crashed. A device
  with no connection at all has most likely lost its own network and keeps the 2 minutes. A device that
  crashed and starts again within those minutes learns the room from the others, still
  listed inside by its earlier session: its room tracker knows when this session started
  and ignores its own joins from before, and the node sends a `Leave` for that room once,
  so every member drops it at once (not when this device is in that voice room again by
  then: the leave would take it out of it too). When nobody is
  left, the room is gone. If everyone drops at once, each keeps its copy until then; a
  participant that comes back later opens the room again with the same id.
- Two members that start a room at the same moment merge them: the earlier start wins, ties
  by id, and events for either id reach the same room (the last 8 other ids are kept).
- Any member can send a room's signed events again later. A `Join`, `Here` or file dated
  more than 10 minutes before the current room started belongs to an earlier room and is
  dropped, so it attaches neither that room nor its chat; an older `Join` or `Here` of a
  participant does not undo a later change of name, and a stop of a share older than the
  share does not end it. What a restarted device cannot tell apart from the room's own
  events (an earlier room's join while it knows no room) still shows that participant
  until the 2-minute grace ends.
- The rooms follow the group's log. A device that is no longer a member (removed, or it
  left) leaves every room of the group at once, with its files: it could neither say it
  leaves nor be heard from again. A voice room a manager deletes closes at once with everyone
  in it, this device too (the manager's device refuses while it sees people inside, but it may
  not see them all), and its later events are dropped. Friends are told this device is no
  longer in a room.
- Who may enter a voice room follows the group's room rules (5.1): a member kept out of a
  room, or left out of a room for some members, cannot enter it. Its own device refuses to
  join, and leaves the room when a rule reaching it keeps it out (the app tells why: "Ana
  took you out of General for 1 hour"). Every member that knows the rules drops it from that
  room and ignores its events there but a `Leave`, and takes no call signal from it while in
  that room, so a device that does not follow the rules sits there alone with those that do
  not either. The rest of the group goes on as before
  for it: its chat, its files, other rooms.
- A member can take someone below it on the ladder out of a voice room now, with no time: a
  `RoomRemove` room event naming it, which only that member acts on (it leaves, says so with
  its `Leave` and may come back at once), and only while it is in the sitting of the room
  the event names, while the event is new (sent less than 2 minutes ago, and no more than
  a minute ahead of its own clock) and once: a member that sends it again cannot take it
  out again. Older versions cannot read the event and drop it,
  so a member on one stays in.
- The app shows a voice room this device may not enter with a clock and the time left, or a
  lock and "Not for you now", and Join does not go in; a room for some members has a small
  lock after its name for everyone. The person menu (right click or long press on someone in a
  room) adds "Remove from <room>" (in red, asked first) and "Time out of <room>" (5 minutes,
  1 hour, 1 day, 1 week, until someone above them lets them back), for those below this device;
  the members list offers the same time out. The group
  settings' voice rooms give each room "Everyone" or "Only some", with the members to let
  in, and list who is kept out, with "Let back".
- Later: encrypted text for a member who is briefly away, held by the others (text only),
  and the room's file index and media host (4).

### 3.9 Presence

Friends see whether a device is online (5b) and whether it is in a room: a status sealed to
each online friend, sent when it joins or leaves a room and when the friend connects. The
group is included only for friends that are members of that group; everyone else sees "in a
room" without the name, so presence never reveals groups a friend does not belong to. Group
members see the room itself (3.8).

A device can **appear offline**. It stays connected and does everything as before, but its
hello, its hello reply and its status carry a flag that asks friends to show it as offline,
and it tells every member it connects to with a `Presence` member request, and again on
every change. To each member one `Presence` is on its way at a time, and a change meanwhile
goes, as it is then, once that one is answered: two at once could arrive in either order and
leave the member with the older. One the member does not take (it was still checking this
device, say) or that does not get there goes again at the next heartbeat while the member
stays connected. Friends and members then show it as offline (keeping when it was last seen
visible), except in a room it is in, since a room shows who is in it. It is a request
honoured by the application, not a secret: a modified client could ignore it. When the
application quits, the node closes every connection first, so friends and members see it go
at once instead of after the QUIC idle timeout.

A device is **away** after 10 minutes without keyboard or mouse input on the whole
computer, and back with the first input. Nobody chooses it. The application asks the
system how long the computer has gone unused (fogata-ffi `idle.rs`). On Linux under
Wayland it opens a connection of its own to the compositor (`wayland-client` with its
pure-Rust backend, so no system library is linked or loaded) and asks the
`ext_idle_notifier_v1` global for a notification after those 10 minutes: version 2's
input notification where the compositor has it, which counts keyboard and mouse alone and
ignores idle inhibitors such as a video playing, else version 1's. `idled` means no input
since 10 minutes ago, `resumed` means input now; while it says nothing, someone is here.
Without it (an X11 session, a compositor without the protocol, a connection that closes)
the session bus is asked: `org.freedesktop.ScreenSaver.GetSessionIdleTime` (KDE on X11
and other desktops; KDE on Wayland says "not supported"), then GNOME's
`org.gnome.Mutter.IdleMonitor.GetIdletime`. The screen saver's `GetActive` says the
screen is locked: while it is, nothing the system reports counts, not even the
compositor's `resumed` for the password being typed, so a device becomes away 10 minutes
after the last input before the lock. The Flatpak is not given the screen saver's name
(it also offers Lock and Inhibit): there idle time comes from the compositor, the X server
or GNOME's monitor, and the lock is not known. Windows uses `GetLastInputInfo`, macOS
`CGEventSourceSecondsSinceLastEventType`. In an X11 session where no service on the bus
says it (i3 and other window managers), the X server's MIT-SCREEN-SAVER extension does
(libXss and Xlib opened at run time; without them nothing changes). Its connection to the
server is opened, used and closed by a thread of its own, which the window asks with a
time limit: a server that does not answer holds that thread, never a worker of the runtime,
and one question at most waits behind it. Xlib is used from threads besides GTK's only
because the Linux runner calls `XInitThreads` first, before GTK (neither GTK 3 nor the Flutter
embedder does; some builds of libX11 do it when they load, others do not). A session bus
that could not be reached is asked for again every minute. Input in Fogata's own window counts
too, and is all there is where the system says nothing. It asks every 15 s, and every 2 s
while away; input in the window and the compositor's notifications wake it at once. What
friends and members see changes only at the turn of a minute of the clock, so they learn
the minute someone left or came back, not the second. Debug builds take
`FOGATA_TEST_AWAY_AFTER_MS` for a shorter wait (and a quarter of it in place of the minute).

The node tells friends and members only when what they see changes. Away is never shown
while the device appears offline (that wins, and says nothing more) nor while it is in a
room or a call, where someone may be listening without touching the computer: joining
one clears it at once. Friends get it as a trailing byte of the status, written only when
away, so versions before it still read every other status (they refuse an away one, and
it is sent once more at the next heartbeat in case the refusal came from a status that
arrived before the answer to the hello); members get `away` in the `Presence` request,
which older versions skip. A friend's or member's away is forgotten when it is no longer
connected. Away changes nothing else: away people count as online, get messages and
calls, and their pictures are not dimmed; they show a moon instead of the green dot.

### 3.10 Files

- A participant shares a file with a `Share` room event: its BLAKE3 hash (the file's id),
  name and size. The file is copied first, as it is then, into `sharing/` in the data folder
  (sealed with the folder's key when it has one, readable by its owner only), and the id is
  the hash of the bytes copied: members are sent that copy, also when they go on from the
  middle, whatever becomes of the file it came from (written over, replaced, made a link to
  another; a link the person picks is followed that once). Sharing needs room on the disk
  for the copy and 256 MiB to spare, besides the room the copies being made at the same time
  hold (two never each take nearly all that is free); a copy goes once its file is no longer
  shared nor on its way to anyone, and what is left there goes when the application starts.
  So a file shared takes its size twice on the disk while it is shared, and so does a file
  downloaded in a room, which is shared from here too (a copy of the download) until the
  room is left. It serves the
  file only once its room took the share, and not if this device left the room or stopped
  sharing it meanwhile (a share the room refuses is never served), while it is in the room;
  another voice room of the group entered stops what it shared in the last. Each block of a
  transfer goes only if, right before it is written, the file is still shared and the other
  device still a member: a file no longer shared (stopped, or the sharer left the room or the
  group) stops its transfers at once, also while they wait for the pace. A room lists up to 100 files of each participant;
  sharing one more is refused with that reason. Every member keeps each share as sent, with a
  video's first picture (64 KiB at most), so the rooms of a group keep at most 24 MiB of
  shares together from all their participants (a picture too big to show counted all the
  same), whatever modified copies send. The size and length a video's share says it has
  are kept as sent too, and only drawn within bounds (`app/lib/src/fair_size.dart`: a
  shape from 1:2 to 3:1, 16:9 for one that says nothing, a length up to 100 hours), in
  the rooms and the direct chats alike: each room keeps its part, 24 MiB over the rooms the
  group has, so all of them together keep 24 MiB, and a share past that is not listed on
  that member. The parts come from the group's log (how many rooms it has, how many people
  a room takes), the same on every member, and never from what a member holds of the other
  rooms: when a group gets more rooms, each room keeps of what it has the earliest shares
  (by the time their sender gave them, then by id) that fit in its smaller part, from its
  next event on (presences come every 20 s), and lets the others go.
  Each participant shares at most its part of the room's, over the people the room takes
  (3 MiB for 8 in a group with one room: about 45 videos with the biggest pictures, far more
  with the usual ones; 300 KiB in each of ten rooms), so a few cannot keep the others from
  sharing, and a member with eight devices in every room of a group makes the others keep
  24 MiB at most (measured: 80 devices in ten rooms, each sharing all it may, make a member
  keep 26 MiB of messages as sent, also when the first room filled up while it was the
  only one, where each room kept 24 MiB before, 240 MiB a group).

  Members holding the same shares list the same ones as long as no part is full, and a full
  part refuses the same shares on every member that got them in the same order. Past that
  they may list different ones, only in what each shows, never in the log: a member keeps
  no share it refused, so one refused while a part was full is not listed later when the
  part makes room (a share stopped, someone left), while a member that got it only then
  lists it; and shares that come in another order (a member that came later and asked for
  the room's events) fill a full part with others. Within its part, a participant whose own
  shares come in its order (its device sends them one after another) is listed alike
  everywhere. Making every member list the same shares whatever came when would mean
  keeping the shares refused as well, which is what the parts are there to stop, or
  asking for them again from those who have them, a change to the room's protocol. Every member keeps
  each event as the whole message sent, so one much heavier than
  what it says is ignored: a chat line or a share more than 1 KiB past its text and
  nickname, or its name and picture, as the message encodes them, and a join heavier than
  8 KiB. A member sending the heaviest events a room takes (500 lines of the longest text,
  each character escaped, and its part of the shares) makes each member keep about 14 MiB a
  room, 141 MiB for the ten rooms of a group (measured; 1.1 GiB when events padded to the
  largest group message were kept). The chat of every room of every group a device is in
  is kept within 128 MiB together (each line counted as its message, text and nickname,
  and 256 bytes more), some 130 000 honest lines: past it, checked every punch slot (10 s),
  the sender with the most kept loses its oldest lines first, so a member that floods its
  rooms (in any number of groups) loses its own lines and nobody else's while it keeps more
  than they do. A line let go is not taken again when a copy comes back. That is this
  device's memory only: each member answers with what it keeps. The shares stay bounded
  by group.
- Another member fetches it over a stream (`/fogata/file/1`) from any participant that has
  it: the request names the id and an offset, so an interrupted download resumes from its
  `.part` file (named after the file's id too, so two files of one name never mix). The connection's own encryption covers the transfer end to end, also when a
  member carries the connection. The receiver hashes what arrives and throws it away if the
  hash differs. A `.part` file is gone on from only with the holder that sent what it
  holds (noted while the app runs); one left by another holder, or from before a restart,
  is cut away first, so a holder that sends wrong bytes and stops does not spoil the
  download from the next. Holders whose download stopped are asked again only after the
  others, and one that sent something that was not the file is not asked again for it.
  Only verified members are served, and only files shared on purpose. Asked for again while
  it runs, a download goes on as one. A request whose stream ends or is reset before any
  answer is made again, after 0.1, 0.3 and 1 s: copies before 0.1.0-beta.9 drop all but one
  of the streams from one member that come in together (see the `stream` behaviour).
- Pictures up to 10 MB are fetched by themselves into memory for their preview in the chat,
  never written to disk and not shared on; they are dropped when leaving the room. One that
  turns out damaged, or that cannot be fetched after three tries, shows as a file. Saving
  one writes it to the downloads folder from memory. At most 128 MB of them are held at once,
  and a picture is only decoded if its header promises at most 50 megapixels for a JPEG or
  WebP, which are decoded straight at the size they show, and 16 megapixels (64 MB decoded)
  for any other kind, decoded whole however small it shows (profile pictures, banners and
  group pictures: 2048 by 2048), since a few kilobytes can ask for gigabytes once decoded.
  Large pictures decoded whole take turns: at most 32 megapixels of them are decoded at
  once (one larger goes alone), so twenty scrolled into view are not all held decoded
  together. A picture of several frames (GIF, WebP, animated PNG) is drawn from its first
  frame only, wherever it shows. In a chat, a GIF or WebP animation of at most 2048 by 2048
  plays over that frame by the rules of profile pictures' animations (**Animations** in
  5b): at most 500 frames, decoded at the size drawn, while seen and focused, 6 at once; a
  video's thumbnail never moves. A picture picked on
  this computer for a profile, a banner or a group is held to the same 50 and 16 megapixels
  before anything is decoded ("That picture is too large"), and decoded no larger than the
  square or banner made of it needs.
- A download's name drops control characters, path separators and marks that hide in text
  or turn it around (`photo\u{202E}gnp.exe` would show as `photoexe.png`) and is tidied like
  every name (5.1: no padding of blank characters; "file" when nothing shows), and never takes the
  name of a file or of another download in progress. A long name is shortened, keeping its
  extension, so that it fits what file systems take with its `.part` ending. A file is shared
  under a name a room takes in the same way: Windows and macOS allow names far longer than
  the 255 bytes a room lists. Dots at the start are left out (`.bashrc` saves as `bashrc`),
  so nothing lands hidden, nor as a file a program reads for its settings when the
  downloads folder is a home folder. A file that acts on its own when a file manager shows it
  or when it is opened, whatever marks it carries, gets `.txt` added (`share.url` saves as
  `share.url.txt`), whatever the case of its ending and with the dots and spaces Windows
  drops at the end left out first: Windows shortcuts, libraries, search connectors, themes
  and the like (`.url`, `.lnk`, `.scf`, `.library-ms`, `.searchConnector-ms`, `.theme`...),
  which Explorer reads to draw their icon and so may reach a network share named inside,
  giving away the person's sign-in; Linux launchers (`.desktop`); macOS terminal commands and
  location files (`.command`, `.terminal`, `.webloc`, `.fileloc`, `.inetloc`...); and the
  files that set up the folder they are in (`desktop.ini`, `autorun.inf`). The list is one
  table in the core (`active_files.rs`), and every place a name someone else gave becomes a
  file follows it: room downloads, attachments of direct messages, and pictures and videos
  saved from the chat, which the core saves too. Attachments kept in the data folder keep
  no such ending either. On Windows the file is marked as
  downloaded from the internet, so the system asks before running it, and so are pictures
  and videos saved from the chat; on macOS the application asks the system to mark
  everything it writes (`LSFileQuarantineEnabled`). Where the mark cannot be written (FAT and
  exFAT drives keep none) the file is kept, as a browser keeps it, and the log says so.
- A download's `.part` file is only taken again when it is a plain file (a link there is
  removed, never followed), and what is opened must be the file looked at: the same device
  and inode on Linux and macOS; on Windows it is looked at and opened without following a
  link, and must have the same volume serial number and file index. It is made new where
  nothing is, readable by its owner only.
  The finished file never replaces anything in the downloads folder, not even a file or a
  link that appeared there during the download: it is hard-linked under its name, which
  fails if the name is taken, and goes under the next free name then (a rename once
  nothing is seen there, where hard links cannot be made). Saving a direct message's
  attachment works the same way, and the copies a direct chat keeps are readable by their
  owner only, and kept only when the bytes copied hash to the attachment's id (the file
  did not change between the two). A video fetched to play is taken again from the application's folder only
  when its contents still hash to the file's id. Asked for again into the same folder while
  it comes (its view closed and opened again), the same fetch is waited for, and in this
  process a download's
  `.part` file is written by that download alone; the fetch stops, its `.part` removed,
  once this device is in no room of the group (it looks twice a second). A room download
  or a video fetched to play starts only when its disk has room for the size its sharer
  gives, and 256 MiB to spare.
- A video is read or played only when its first bytes are what its name says: MP4 or
  QuickTime (`.mp4`, `.m4v`, `.mov`), Matroska or WebM (`.mkv`, `.webm`), AVI (`.avi`).
  The application says which first bytes fit the name (`videoHeads`), and the player itself
  opens the file once, checks them on what it opened and reads the video from that same
  open file, so nothing can be put at its path between the check and the reading: on Linux
  GStreamer reads `/proc/self/fd/<n>` of the descriptor checked (a pipe or a device is not
  opened at all); on macOS AVFoundation reads through a resource loader of the plugin's,
  from the descriptor, at most 2 MiB an answer, so a long video never sits in memory; on
  Windows Media Foundation reads a byte stream over the handle checked. Openings and readings
  whose file never opens (a network share that hangs) are bounded in number.
  On Linux, videos play with GStreamer limited to the demuxers of those formats, with no
  network elements and no subtitles, so a file that is really an HLS or DASH playlist does
  not make the player fetch what it lists; and only the decoders and parsers of VP8, VP9,
  AV1, H.264, H.265, MPEG-4 Part 2, MJPEG, Opus, Vorbis, AAC and MP3 (software ones) take
  part, not every decoder installed. macOS and Windows keep to the same codecs: before a
  first picture is decoded or a player made, the tracks' codecs are read (AVFoundation's
  format descriptions, Media Foundation's native media types) and a file with any other is
  not read nor played, since decoders are the most error-prone code a received file
  reaches. HDR videos (PQ or HLG, as phones film them) are tone
  mapped there, in the plugin (`fogata_video/linux/tone_map.h`): their pictures come at 16
  bits a value, each becomes light (reference white at 203 cd/m²), BT.2020's colours become
  BT.709's, and light past the screen's white rolls off instead of being cut; the
  thumbnail too. Shown as they were, they looked washed out. macOS and Windows convert
  them in the system's own player.
- Videos play only up to 8K pictures (7680 by 4320 pixels, no side over 8192), on every
  system, and a call's camera or screen frames larger than that are not drawn: each picture
  is copied at four bytes a pixel, and a file or a peer can claim up to a gigabyte a frame.
- On Windows a video's first picture and length are read with Media Foundation's
  asynchronous source resolver and source reader (`fogata_video/windows/video_probe.cpp`):
  opening the file, choosing the decoder and decoding the first picture get 10 seconds in
  all; what is still pending then is cancelled (the open, or the read, flushed) and
  everything is let go of, so a file that stalls the decoder or never finishes arriving
  does not hold a thread, or one of the two turns to read videos, for ever
  (`windows/test/run_tests.ps1` checks it).
- A device sends a member a file, or the rest of one that stopped, as fast as the line takes
  it, whatever its size, and never cuts one off on its way. What it sent that member of a
  file in the last day (or of any file, past the last 256 it remembers for it) goes again
  only as fast as the line takes it up to 512 MiB, then at 1 MiB a second, what it was not
  sent coming back to it at that pace, so that one member asking again and again leaves
  most of a home's upload to calls and to the others. A file going at that pace gives its
  place among the 32 files sent at once back, for one of 8 places kept for those (one per
  member), so members asking again never keep the others' downloads waiting. Members are
  counted by the device they proved to be, under whatever connection they come (4 files sent
  at once to one device, 1 at the pace). A request waits 20 s at most for a place, less than
  the asker waits for an answer: then it is told to ask again in 10 s, as when its day is
  over, and its download waits rather than fails. A device whose files are taken at less
  than 4 KiB/s together over two minutes of waiting for it (one block every half minute
  holds a place for the whole of a large file otherwise) gives each place up while another
  device waited for one in the last two minutes; nobody else waiting, a slow line keeps its
  places. 4 KiB/s (32 kbit/s) is a third of the slowest honest lines seen (a satellite dish
  at 0.1 Mbit/s, mobile data far from a mast), shared by four files at once, and a stall of
  most of a minute in a window does not count. The device goes on from what came: a
  download that stops after a block or more came (`FileError::Cut`) is asked again from
  there, a room's without using up one of its three tries, a direct message's file the
  person asked for until it comes. A device is
  sent at most 32 GiB a day of everything (at the pace alone it could take 84 GiB): a file
  that starts within its day goes whole; after it, it is told when its next day begins, and
  its download waits until then and goes on (a room's download, a direct message's file
  asked for), or is tried again then (one that comes by itself); a picture's preview fails.
  The count is kept in memory: it starts again when the sending device starts.
- After a download the receiver shares the file too, so it stays available when the first
  sharer leaves: from a copy made then, as when sharing, if it still hashes to the file's
  id and the disk has room; and only if it is still in the room. When no participant that has it is left in the room, it disappears from the
  list. Nothing is uploaded anywhere: if nobody has a file, it is gone.
- Later: fetching parts from several holders at once, and a size limit per room.

### 3.11 Direct calls

A friend online can be called directly. The caller makes a group with just the two of them
for the call (a new one each time), enters that group's room and rings: a `Call` friend
request, sealed to the friend, carries the group's log. The friend's device checks that the
log is the caller's (it made the group), with exactly the two of them, and rings for 30 seconds (40 at most, in
case the caller's "stop" is lost). Answering adds the group and enters its room, so the call
is a room of two with everything a room has (voice, cameras, screens, chat, files, volumes);
nobody else can enter. The call counts as answered only once this device is in the room
and the caller was told: one whose room cannot be entered ends as not answered, and the
caller hears it cannot be now. The caller likewise rings only once in the room. Declining, being in another call (busy), appearing offline (declines
without ringing), stopping the ring and hanging up are further sealed `Call` steps; leaving
the room or the friend going offline also ends a call that rang or went on, and so does
removing the friend, on either side (a group's voice room is no call and goes on). Two
friends who ring each other at the same moment do not each hear that the other is busy: the
call with the lower id goes on and the other device answers it at once (an older device
still answers busy); one whose friend's ring is already there when it calls answers that
ring instead. A device takes one call at a time: calling a friend while another one
rings here declines that ring first (the app asks), and answering ends a call it rang or was
in. The call's group is never saved, listed or offered to friends as a
group, and the node lets it go when the call ends. A ring that stopped before an answer is a
missed call for the one called, and so is a ring answered busy (the person sees that the
friend called while they were in a call) and one declined while appearing offline, which
comes from a friend who had not heard yet (the person sees that the friend called while
they appeared offline).

A call goes on while anyone is in it. When someone hangs up, the others stay; the last one
left waits a minute for the others to come back, and the call ends if nobody does or that
one leaves too. Someone who left can come back while the call goes on by calling one who is
still in it: the ring carries the same call's group and is answered at once. A ring or an
invitation under the id of a call this device is in, or left a short while ago, is taken
only from someone in that call (a member of its group as this device knows it, or a friend
being rung into it from here): a friend that learnt the id could otherwise make up a group of
its own under it, be answered as one coming back and told since when the call goes, or take
the place of the call left here. Those still in
a call tell each friend who left it that it goes on, with a `Going` step (how long ago the
call was answered, and for how much of that two or more people were in it), when someone
comes or goes and every 15 seconds, and they send `End` when they leave it. So the device
that left knows which of its friends are still in the call, and its Call buttons for them
say "Return to call" (in the chat, on the card, in Friends, on a missed call); a friend
silent for 40 seconds, gone offline or that said it left no longer counts, and neither does
a word of its that arrives out of order just after it left. `Going` also tells someone who
comes into a call since when it goes; a start said to be before the call's group was made,
or more than 48 hours ago, counts as that.

When a call is over for a device (it was the last one, or no friend of its is left in a call
it left, or the application quits), it notes the call in its chats: a line such as "Ana
started a call that lasted 25 minutes" ("a few seconds", "1 minute", "1 hour 20 minutes"),
kept with the chat and sealed like its messages, but never sent: each device writes it from
what it saw, so both sides of a call of two show the same starter and length within a
second. The length runs from when the call was answered to the last moment two or more
people were in it; the minute the last one waits alone does not count. A device notes the
call with each friend it saw in it who started it, with all of them when it started it, and
with those it rang into the call or that rang it in (under the starter's name when that was
neither of the two), so each line is on both devices of a chat. Missed and declined calls
stay in the bell, as before. Anyone in a call can
ring friends into it, up to eight people in a call, those being rung included: the friend
is added to the call's group (everyone manages it, so each can add their own friends) and invited with an `Invite` step, which carries the group's
log. The eight count those in the call now and those being rung, never those who left it,
though they stay members of the call's group. An invitation is taken only with the one
inviting and the invited device among the members. Into a call the device is in, or left a
short while ago, it is taken with no more members than a group takes (20); into a call it
was never in, only when the one inviting is the one who added it to the call's group (a
friend cannot ring it into a call someone else made) and with no more members than a call
takes (8), so nobody new is added to a call whose group has eight members, those who left
it included. Until a member's own profile comes, the call shows the name the one who added
it gave it, followed by "named by" and who that was, unless it is a friend. People in a call
see each other's names and pictures as in any room, friends or not.
Each side of a call tells the other with a `GroupCalls` step that it takes all this. An
older device refuses both steps: invited, it is shown at once as too old to join; in a call
of two with a newer one, nobody can be added, and its call ends as before when either
hangs up (the newer one waits its minute, and the older one's call back rings as a new
call).

An identity open on two computers at once (a copied data folder) has two sessions a friend
can reach. A ring goes to every session; the first to answer or decline takes the call, and
the caller tells the others the call was taken elsewhere (`Elsewhere`), so they stop ringing
and show nothing missed; an `Elsewhere` from anyone but the one who rang is not taken. The friend is busy only when every session rung said so. The
caller's later steps go to every session, and the answers of the one called go back to the
session that rang. A device older than `Elsewhere` refuses it and is told the ring stopped
instead, which it shows as a missed call.

### 3.12 Network check

`fogata network` starts a node with no group and no friends, talks to the public DHT for 15
to 40 seconds and explains, in plain words, what it found and what that means for reaching
friends: whether the DHT answered, whether UDP works (observed addresses only come from
QUIC), whether the device has a public IPv4 address or is behind a NAT that keeps one port
for every destination (easy) or changes it per destination (hard), a carrier-grade NAT
address on the device, public IPv6, and other devices on the local network. It is the base
for the app's "connection failed" screen. In the network lab it tells an easy NAT, a hard
NAT and a public address apart, and a network that lets no UDP out (`network-check`); a
device whose UDP is dropped towards two of the three DHT nodes only still finds that UDP
works, and one with no way out at all finds that nothing answers (`network-check-negative`).

Nothing in it is guessed, and nothing old is taken for now:
- **Contact.** Only a peer at a public address that connected or answered a ping counts as
  the internet answering; the DHT's table fills from the bootstrap list alone. Nothing on the
  internet answers when, 30 s or more on this network, nobody did in the last minute and at
  least two dials to public addresses failed since (no connection, or a firewall letting
  only web traffic out).
- **UDP** works while a QUIC connection with a public address opened or answered a ping in
  the last two minutes. That no QUIC connection opened shows nothing by itself: some DHT
  nodes answer only over TCP, and a dial to several addresses stops at the first that
  answers. So the node asks a few peers it dialled itself over TCP (never one that came in)
  for a QUIC connection too, at the IP of that TCP connection only and at most two ports
  (any other address would be one the peer chose, anywhere), at most six peers per network,
  only while UDP is not known to work (`node/udp_check.rs`). A dial that timed out or that
  the system refused to send counts once a ping over the peer's TCP connection answers after
  it; three such peers, and no check dial still going, mean this network or this computer
  blocks UDP. Any other failure (a wrong peer, a refused handshake) proves nothing, and none
  of these dials counts against a DHT candidate.
- **A hard NAT** takes peers reporting one public address with different ports: two ports
  each reported from two addresses, or three from three addresses.
- **IPv6** goes around a router only when this device has a public IPv6 address and peers
  reached it there.
- **A new network** (an interface joined or left, a new public address, a wake-up) forgets
  the router, UDP and contact; peers still connected over TCP are asked over QUIC again, and
  a check dial of the old network counts for nothing. A temporary IPv6 address expiring
  while another of its /64 stays is no new network.

The app says the same, briefly, where a connection failed: on the card of a friend who is
offline, on a room that could not connect and on a call that could not reach the friend. It
asks the running node what it saw (`reachability`, at once, without the check's wait) and
again every 10 seconds while shown, and names only what is in the way on this side: this
computer's firewall letting nothing in (3.3), nothing on the internet answering, UDP
blocked, or a hard (symmetric) NAT, then with no working IPv6 either. Only with an answer
from the internet in the last minute, UDP working and a router that can be opened (or none)
does it say the network looks fine, so what keeps the two apart is probably on the friend's
side (offline, or their network), or for a room or a call on the others' side; a friend's
card leaves that sentence out, since it already says the friend is offline and when they
were last seen, and names only what is in the way here. When the node has not seen enough
for either, it says nothing. No relay or server is involved: this
only explains.

### 3.13 Direct messages

A friend can be written to directly, as in a chat application's direct messages. Each chat
runs over a group of just the two friends (the chat's group): a `Create` and an `Add` of the
other, made by whichever writes first. So a chat has what a group has, key epochs, the
membership proof and sealed, signed messages (3.6, 3.7), and only the two devices can read
or write in it. A chat's group is never announced or looked up on the DHT, never sealed into
local addresses, never offered as a group and has no rooms: its log goes to the friend over
their friend connection (`FriendRequest::Direct`, once per connection while the friend has
not proved it), and the two prove membership over that same connection whenever both are
online; a device asks no rosters of it and keeps relay slots, sends its local addresses,
profile and presence only to members of other groups. A log offered as a chat is taken only
if it has exactly the two, was made by one of them, opens with a key this device holds and
is none of its groups or calls. Two made at once (both wrote while apart) settle on the one
with the lower id; the other is let go. A chat's group whose log stops fitting (someone else
added) is let go too, and a new one is made when someone writes.

- **Messages.** `MemberRequest::Direct` carries up to 64 sealed messages for the other person
  only; nothing is flooded. Their plaintext (`DirectPayload`, JSON) is a message (an id, its
  author's time, text of at most 2000 characters, up to 10 attachments with their BLAKE3 id,
  name, size, kind and a video's first picture, and how long it lasts), a change of the
  disappearing timer, a pin or unpin of a message, or a receipt. A call that ended is a
  line of the chat each device writes for itself (3.11) and never sends; it disappears
  with the chat's timer like a message. A device keeps sending its
  messages, when the chat's
  group connects and every 15 to 20 seconds after, until the other's receipt says they are
  kept there; whenever one goes, those still waiting for a receipt go again before it, so
  they arrive in the order written even when one went into a connection already gone (as
  after the computer slept). Something new (a message, a change of the timer, a pin) is written only while
  both show as online: the core refuses it while the friend is offline or appears offline,
  or while this device appears offline, and the chat shows why in place of the message box
  ("Marta is offline. You can write when you're both online."); what was received stays to
  be read, and an attachment not fetched yet says it is available when the friend is
  online. What was written before and still waits for a receipt (the friend left in the
  meantime) shows "Waiting for Marta to come online · sent when you both are" and goes by
  itself once both are online. A copy that
  arrives twice is kept once. What arrives is checked (lengths, names without control
  characters, hiding marks or path separators that show something, sizes, known timers);
  names are kept as they show (5.1); an attachment shows as a picture or a video only if its
  name says so too, since what plays a file is chosen by its name; what does not fit is
  confirmed and dropped. A device that appears offline keeps what comes but sends no
  receipts, which would show it online; the friend keeps sending. Nor does a device that
  cannot save what came (a full disk, a folder it may not write): it is confirmed once it
  is saved, when the friend sends it again. What the person writes while it cannot be saved
  is refused with the reason and not sent, so no message is on the friend's device only.
- **Order and unread.** Items go in the order they reached the device, a device's own as
  they are written: an item from the friend goes after everything that was there when it
  arrived, so a friend's clock behind never puts a reply above what it answers. What waited
  while the two were apart comes in one go, in the order the friend wrote it. Each device
  sends a chat's requests to the other one at a time, the next once the one before is
  answered: requests on their way together can arrive in any order, and two messages
  written a moment apart would show swapped. When one does not get there, those after it
  wait to go again with it, in order. The time shown
  is the author's, kept between the item above's and when the item arrived, so it never goes
  backwards down the chat and a clock ahead shows the arrival. Journals from before this
  order load in it too (their items already counted the order they arrived in). The list
  of chats goes by when each chat's last line arrived (or was written), so a chat that just
  got something comes first. The friend's messages that reached the device after the last
  one read are unread.
- **Attachments** travel over the file protocol (3.10) from the other device, served under
  the chat's group to its other member only, while both are online. Pictures up to 10 MB
  come as they arrive, videos and files when they are opened (each a setting). What comes
  by itself has a daily allowance per friend device, 512 MiB of pictures, videos and files
  (some fifty of the largest pictures that come by themselves), charged once per file as it
  starts (`Record::FileCharge`, so a restart does not refill it); past it they wait for
  Download. Room for what comes by itself is made in its own chat only, its oldest
  attachments (pinned ones last): what other chats keep is never pushed out by a friend's
  pictures, and one that would need it is not fetched, and waits for Download. A picture is
  kept as one only if its file starts as a PNG, JPEG, GIF, WebP or BMP (checked as it
  arrives); one that does not becomes the file it is (`Record::NotPicture`), kept when it
  was asked for or files come by themselves, otherwise thrown away. At most four of a
  friend's attachments that come by themselves, and sixteen of all chats, are on their
  way at once; the others wait for their turn (what is asked for goes at once). Each
  download that comes by itself is given up past two minutes and a second for every
  32 KiB of the file, so a friend who sends slowly holds no stream or file open for long;
  one that was asked for takes as long as the link needs and is given up only when
  nothing of it came for two minutes. What came stays for the next try to go on from; a
  sealed one is taken up from it (read and checked whole) away from the async threads,
  which counts as coming, and keeps its `.part` file to itself until that is over even
  when the download is given up meanwhile, so the next try never takes it up at once.
  The sender
  keeps a copy in the chat's folder, so it can go later. A file sent again is one file
  (its id is its hash): the copy already kept serves every message that carries it; one
  whose name ends otherwise (`a.jpeg` after `a.jpg`) finds it under its own ending too, a
  hard link to it (a copy where the disk has none), and removing the file removes it under
  every name the chat knows it by. Each
  device has one limit for all its chats (500 MB, 1, 2, 5 or 10 GB, or none; 2 GB at
  first), which counts their attachments and their journals (the messages, which always
  stay): when attachments alone are past it the oldest go first, by their message's time,
  and those of pinned messages only after all the others, when those are not enough; the
  message stays and says so ("Removed here to free space"), and the attachment can be asked
  for again while the friend has it. What the journals put past it is made up only by the
  chats that take more than their share of the limit (the limit by the number of chats),
  each by what it is over and what its journal takes at most, and only from the files its
  friend sent that are not pinned (`model::to_free`): a friend's messages never push out
  what other chats keep, nor pinned files or the files this device sent. Messages that fill
  the room that way stay past it, and nothing comes by itself until there is room again
  (what comes by itself must fit beside every journal and the other chats' attachments). An
  attachment is fetched, as a room download is, only when the disk has room for it and
  256 MiB to spare: one asked for is refused at once, one that comes by itself waits and is
  tried again later. The same holds for every other file this device writes from a chat:
  the copy of a file it sends and of a voice message recorded here (which stays in memory,
  at most 4 MB, until it is sent; one refused stays to send again), a copy saved elsewhere
  (in that folder's disk), and the plain copy a sealed video plays from (removed when it
  stops halfway, as a saved copy is). Refused, the person reads that there is not enough
  free space for the file. A chat's files can be cleared by hand, pinned ones too; the
  friend keeps its copies. Text always stays. Only a video keeps the first picture its
  message carries. What a friend's items (messages with what describes their files, timers
  and pins) take in the chat's journal, and in memory, is bounded too: 16 MiB a day per
  friend device, and 256 MiB in all in the chat (`RECORDS_MAX_BYTES`: a million messages
  of text, or some three thousand videos with the largest first picture); past either the
  next ones are not confirmed, and wait on the friend's device, which sends them again
  until they fit (after messages here disappear or are deleted, for the bound in all).
- **Voice messages** are attachments too: an Ogg Opus file (RFC 7845) named
  `Voice message.ogg`, of kind file, whose description in the message also carries
  (`DirectFile::voice`) its length and 40 levels from start to end, so the message shows
  its bars and length before the file arrives. They come by themselves, whatever the
  settings say for files, up to 64 MiB a day from each friend (about 25 of 15 minutes);
  past that a friend's voice messages wait for Download as files do, so nobody can fill a
  computer with them. Every one that comes by itself is charged once a day, at least
  64 KiB (so a burst of tiny ones fits no more than 1024 a day), as it starts
  (`Record::VoiceCharge` in the chat's journal once its first bytes arrive, so a restart
  does not refill the allowance), whether it then comes or turns out not to be a voice
  message; one that does not start (the friend's device is gone, or no longer has it) is
  not charged. One past the allowance is not started. One that did not come by itself is
  tried again by itself (every 20 s while the friend is connected) only after a wait that
  doubles each time, from half a minute up to an hour; pictures, videos and files that come
  by themselves are charged and tried again the same way. The allowance is per friend
  device: a friend with two devices has two. Charges older than a day count no more and are
  dropped as the chat is read and whenever its journal is written whole; a journal that
  still holds one more than two days old is written whole again (checked every minute),
  so when and how big a friend's attachments were does not stay on the disk. A file that came by itself and does
  not read as an Ogg Opus voice message of the length its message says (within a second;
  checked once, as it arrives, also when the file is here already from another message,
  so a file sent back as a voice message is not shown as one) becomes the file it
  is (`Record::NotVoice`): kept when files come by themselves, otherwise thrown away and
  shown with Download. Room for one that came by itself is made in its own chat only, as for
  pictures. They count as
  files in what a chat takes; they disappear, are freed and cleared as any attachment. Copies from before them skip the description they do
  not know and show a file of that name. What the friend says of one is checked (an `.ogg`
  name, at most 4 MB, from a moment to 15 minutes and a second, 40 levels): anything off
  and it shows as the file it is. Playing one here is noted (`Record::Played`), so the dot
  of a friend's message not played yet goes for good. The file is read as untrusted
  input by `fogata-voice` (below) when it plays.
- **Disappearing messages.** Each chat has a timer: off, 24 hours, 7 days or 90 days, with a
  default for new chats. A change is an item of the chat, shown as a line, and the timer is
  the latest change by its author's time (ties by id), the same on both devices once both
  have every change; a change made here goes after every change known here. A change from
  the friend dated more than a day after it arrived is confirmed and dropped (as is such a
  pin), or it would win over every later one for years. Each message
  carries the timer in force when it was written, and each device deletes it, with its
  attachments, that long after it reached that device; so a wrong clock on one device never
  removes messages early on the other. A message whose time ran out before it was delivered
  is never sent. Hovering a message of a chat with a timer tells when it goes from this
  device ("Disappears in 3 h"); no countdown shows on the messages themselves. When messages go, the chat's journal is written again, so their text does
  not stay in the file.
- **Pinned messages.** Either friend can pin a message, or unpin it, for both. A pin or unpin
  is an item of the chat (`DirectPayload::Pin`: its id, its author's time, the message's id
  and whether it is pinned) that travels and is confirmed like a message; a pin shows as a
  line ("Ana pinned a message", which brings the message into view), an unpin shows nothing.
  The latest pin or unpin of a message by its author's time (ties by id) counts, the same on
  both devices once both have every change; one made here goes after every change of that
  message known here. Only that latest change is kept, even an unpin, so pinning and
  unpinning a message again and again does not grow the chat, and an older change that
  arrives later (sent again, or from an older journal) is confirmed and dropped. So the
  chat shows one "pinned a message" line per message pinned now, where it was last pinned. A pinned message does not disappear; unpinned, its own time counts
  again, and if that has passed it goes at once on both. Pins go with their message. A chat
  keeps at most 50 pinned messages: past that a pin is refused ("unpin one first"), and if
  both pin at once and go past it, the 50 pinned last count on both. A message pins only
  while both devices still have it. A device refuses to pin one of the friend's whose time
  ran out by when the friend wrote it, where the friend counts it from ("that message has
  already disappeared for Bea"); and a pin of a message the device no longer has (its time
  ran out there before the pin came, as when the other device got it late) is confirmed and
  answered with an unpin, which travels like any unpin, so the other device does not keep
  the message pinned, alone, for good: there it goes on its own time. Pins and unpins go only to a device that
  takes them: one that lists `/fogata/direct-pins/1` among its protocols (identify; no stream
  of it is ever opened), or `/fogata/typing/1` (copies from before the marker that take
  typing signals, which came after pins), or that sent or confirmed a pin since it
  connected. A copy from before pins cannot read the payload and would skip it without a
  receipt, so it is never sent one: the pin waits on the device that made it, which shows
  it pinned, and goes when that copy is updated and connects again; meanwhile their
  messages and receipts work as before.
- **On disk**, `direct/` in the data folder: per friend (its device key in hex) the journal
  (records for an item, items delivered, read, an attachment fetched or removed, items gone,
  and the two no longer being friends), `pins` (the same records for pins and unpins, apart,
  so a copy from before pins reads the journal whole and leaves them alone), the chat's
  group's log, the friend's profile as last known and the attachments by id; and
  `settings.json`. With a data key (5c) the journal and the pins start with a sealed file's
  header and each record is sealed on its own, so adding one stays cheap, as a chain: each
  record is bound to the one before it and to its chat, so records taken out of a file's
  start or middle, repeated, put in another order or moved to another chat do not read
  (records sealed each alone, as they were before, are read and written again as chains at
  start). How each of the two ends (how many records, and the last one's tag) is kept in
  the chat's `tails`, sealed whole for the chat, written after each change (while the two
  are written whole again, how they will end is kept first, so a write cut short reads as
  either): records cut off the end, an older copy of a file put back, a record that does not
  read, or tails that do not open, show at the next start as a notice in the chat ("This
  chat's history on this computer changed since Fogata last saved it. Some messages may be
  missing."), for that start; what is there then counts from then on. A folder from before
  tails were kept counts from its first start. The whole folder put back, tails with it, is
  not noticed. The friend's
  profile is sealed whole and attachments as streams, which are served, saved and fetched
  through their seal (a download is sealed as it arrives, its `.part` file too). The screens
  get a picture's bytes from the core, and a video plays from a plain copy in
  `direct/open/` of its own, under a name of its own, that goes when its player closes and
  at every start. A damaged journal keeps what came before the damage and a copy is put
  aside (a damaged group log keeps every entry that reads, past the damage too). Part of a record that a full disk cut short is taken off the end
  at once, and the journal and pins are written whole at the next change, so what comes
  later never lands behind a broken record.
- **No longer friends.** When either removes the other, the chat stays on both devices, can
  only be read, says so, and its group stops; each can delete it on their own device.
  Becoming friends again resumes it. Removing Fogata's data, or losing the device without
  another, loses its chats: no copy is kept on any server or on the DHT.

### 3.14 Typing indicators

Under a chat's messages, "Ana is typing…", "Ana and Leo are typing…" or "Several people are
typing…". Each signal says that its sender is typing in a chat now, or stopped; it is never
kept, never part of a group's log or a room's events, and nothing forwards it.

- **Wire.** One stream of `/fogata/typing/1` per signal and recipient, 30 bytes: a version
  (1), typing or stopped, the chat's group id, the voice room (zeros in a direct chat) and
  the sender's count of signals, which only grows in a session. A receiver reads at most 64
  bytes of a stream (a later version may add to the end) and ignores another version.
- **Who gets it.** In a direct chat, the other person, while connected as a member of the
  chat's group; in a room's chat, the members connected now that are in the same voice room
  as the sender. Only devices that list `/fogata/typing/1` among their protocols (identify)
  get one, so a device of an earlier version is never sent anything; it simply shows no
  indicator, and sends none. A device that appears offline sends none; one that starts to
  appear offline while typing says "stopped" first in each chat it was typing in.
- **When.** The application says "typing" when the text in the message box changes and is
  not empty, at most once every 3 seconds (the node also keeps 2 seconds between two for one
  chat), and "stopped" when the box empties (the message went, or it was cleared) or closes,
  once, and only if "typing" went in the last 10 seconds.
- **What arrives.** A stream is read only from a peer that proved membership of a group
  here, two at once from one peer and 32 in all, for at most 5 seconds. A signal is shown
  only from a peer that proved membership of that chat's group and is still a member of
  it, about a direct chat or the voice room this device is in, with the sender in it; at
  most four at once from one peer and one a second after that; and only if its count is
  higher than the last one heard from that peer in that chat, since signals on separate
  streams may overtake each other. What the node keeps of it (the counts and allowance of
  each connected peer) goes when the peer disconnects.
- **Shown.** Someone shows until 6 seconds after their last signal, until they say they
  stopped, until their message arrives, or until they leave the room or go offline. The
  line under the messages always takes its place, so the messages never move.

### 3.15 Capability levels

Each version of Fogata's protocols has a level: every version before levels is 1, the one
that brought them 2, the one of level three of the group log 3, this one 4 (level four of the group log, 5.1). A device lists `/fogata/caps/<level>` among its protocols (identify); a stream opened on
it is closed at once, so nothing new goes on the wire and earlier versions only see one more
protocol name. A peer that lists none is level 1; the highest level named counts. Like every
Fogata protocol, it is listed only to members and friends: a peer that dialled this device
learns it once it proved itself, by a push.

Once a peer is known to be a device (a proof of membership, or a friend's session), the
node remembers the highest level that device was seen at, and the application keeps that
list in `levels.json`, a file of its own that earlier versions do not read, and gives it back
at the next start. A feature that came with level N and keeps something safe is used with a
peer that says N or more; with a peer that says less it falls back to the older way only if
its device was never seen at N or more; otherwise it does neither (`caps::decide`). So a
modified copy cannot pretend to be an older version to get the weaker behaviour from a device
that knows better. The level a whole group can use is the lowest of its members'. What the
group's log keeps and applies is judged at a level of its own, raised with a `Level` entry
of a manager, never above the level every member speaks (5.1).

## 4. Voice and video

### 4.1 Topology

- **Mesh** up to 8 participants, cameras included: a camera goes out smaller the more
  people there are, and smaller still to someone whose connection falls short (4.3).
- **Host** above that: everybody sends one stream to the host, which forwards packets
  without decoding. The host is chosen deterministically from shared metrics (upload,
  round-trip times, direct paths, power source); a backup host keeps warm connections.

### 4.2 Transport and encryption

Measured in the network lab (see `tools/netlab/netlab.sh` probe scenarios and
`tools/media-lab/`):

- Reliable streams are fine for voice (with 3 % loss about 3.6 % of packets arrive more than
  100 ms late, almost exactly the lost ones) but not for video: at 1.2 Mbit/s with 1 % loss,
  7.6 % of packets were over 100 ms late, and at 2.4 Mbit/s delays reached 1.4 s.
- A new stream per packet is unusable (seconds of delay: every stream costs a round trip).
- QUIC datagrams with the default Cubic congestion control still queued video for up to half
  a second, because Cubic reads random loss as congestion.
- QUIC datagrams with BBR: voice and video up to 2.4 Mbit/s arrived within 36 ms with 1-3 %
  loss, losing only what the network lost.

So media uses unreliable QUIC datagrams with BBR; chat and control keep reliable streams.
rust-libp2p disables datagrams, so `vendor/libp2p-quic` is a patched copy (MIT) that enables
them, exposes them per remote peer (`datagram.rs`), keeps a small datagram send buffer, and
selects BBR. Later measurements showed BBR takes media as limited by the application and
never holds it back, so the send buffer hardly matters: WebRTC's own congestion control does
the work. Any peer that completes a handshake (a DHT node, a stranger) can send datagrams:
those of peers that are not verified members are dropped before they reach the media
bridge's queue (4096 packets), and one member's packets take at most a quarter of it, so a
flood from anyone else leaves members' voice and video their places. The check that a peer is
a member uses a node handle that does not keep the node running, and the process-wide queue
lets go of it once the bridge's tasks end, so a core dropped after a call still stops its node.

A datagram must fit in one QUIC packet. At QUIC's smallest packet (1200 bytes) that leaves
1148 bytes for a WebRTC packet, less than a full video packet (about 1210 bytes) or a DTLS
handshake packet; a 1280-byte path (some VPNs and tunnels) leaves about 1180. quinn also
falls back to its smallest packet for a minute at a time when it takes reordered or bursty
losses for a black hole, which jitter on Wi-Fi does. A packet larger than what fits goes as
two (at most four) datagrams with a short header each (both addresses, a 16-bit number,
place and count), and the receiving bridge puts them back together; a packet whose parts
do not all come within 300 ms is dropped, and at most 256 wait for their parts, 64 from
any one member. A lost part loses the packet, which WebRTC asks for again or does without,
as with any lost packet, so nothing waits behind it. Only bridges that put parts back
together get them: each device takes streams of `/fogata/media-parts/1` from the moment its
node starts (none is ever opened), so the protocols it lists to others say so, and the
sender checks what the member listed; an older member still gets such packets on the media
stream, as before. Before, such packets went on the member's media
stream (below), where a loss held up every packet behind it and packets that waited too
long were dropped; resent video packets are full size too, so the screen could stay
stuck. In the lab, under jitter, between a tenth and a third of the video took that way;
over a 1280-byte path with 3 % loss and 90 ms round trips, one run in five of a moving
1600x900 screen ended under a frame a second (the estimate at 130 kbit/s, the encoder at
30, the round trip steady) and another at 13; in parts, four runs out of four at 26. The
debug summary
(`RUST_LOG=fogata_media=debug`) gives every 5 s, per member, what went as datagrams, in
parts and on the stream, what came in, and the QUIC path's state. Audio is small enough to
go whole.

When a member has several QUIC connections (a new one after a network change, while the old
one has not timed out yet), media takes the one that received a datagram last, or the
newest when none did within two seconds. QUIC connections time out after 30 seconds without
a packet (keep-alives go every 5), so a call outlives a short outage where it was, and after
a longer one the member is dialled again (3.4); on leaving a network a device dials every
member's QUIC addresses again at once.

WebRTC (flutter_webrtc in the app, any libwebrtc engine in tests) carries the media:

- Each device runs a TURN endpoint on `127.0.0.1` for its own WebRTC engine, which is
  configured with relay-only ICE. Each of the engine's connections is for one member and
  authenticates as a TURN user that names that member's device (the bridge's random user,
  a dot, the device key in hex), so each allocation is that member's: what the engine
  sends through it goes to that member only, and the bridge hands an allocation only what
  that member sends. Allocations get virtual relay addresses at the device's own address
  `10.a.b.c` (24 bits of a hash of its transport peer id, the last byte never 0 or 255),
  told apart by their port; a packet from a member for any other address is dropped.
- The bridge owns every task it runs (refreshing the members' identities, reading
  datagrams and media streams, writing streams, the debug summary): closing it, as the
  core does when it shuts down, stops them and waits up to 2 s for them to end, and a
  start that fails half way stops what it started. Dropped without closing, it stops them
  without waiting.
- Only STUN messages and channel data reach the TURN server, and only so many requests: a
  source the server answered with valid credentials (one of the engine's sockets) has its
  own share (32 at once, 2 a second), and every other source shares one (256 at once, 8 a
  second), so a flood of requests cannot stop the engine refreshing what it holds. Each
  request without valid credentials makes the server keep a nonce; the vendored TURN crate
  drops those past their hour, and the oldest beyond 4096 (`vendor/turn/FOGATA.md`).
- The engine asks for an allocation through every interface the device has (Wi-Fi,
  Ethernet, VPN, Docker bridges), and each becomes a candidate: with two on each side a
  call had four candidate pairs, all leading over the same QUIC connection, and every
  switch between them started the engine's bandwidth estimate over. The endpoint answers
  only requests through the interface the default route goes through (found by asking the
  system for a route, nothing sent), and refuses the others, which fail at once: each side
  offers one candidate and a call has one pair. Without an IPv4 default route every
  interface is answered. On Windows the endpoint listens on that interface's address and
  hears nothing else anyway.
- Sending through a member's allocation puts the packet in a QUIC datagram to that
  member (to the transport identity whose virtual address the engine sends to, when the
  member is connected under several); the member's endpoint delivers it as data from our
  virtual address. No outside STUN or TURN server is involved.
- WebRTC keeps echo cancellation, codecs, jitter buffering and congestion control.
- Each side's description limits the camera it receives to 1500 kbit/s (WebRTC would let it
  take about 2500 per member, which a mesh of four cannot upload; with three people or
  more the sender keeps it lower still, 4.3) and lets voice use Opus
  DTX, so a silent participant sends next to nothing. The members' transport identities
  are fetched again at once when a packet finds none for its member, so a call that starts
  just after a member connected does not lose its first checks.
- Lab results with a browser's WebRTC engine (audio tone plus 30 fps video): with 2 % loss and 30 ms
  delay each way, 1.5 % of audio concealed and no video freezes; with 5 % loss and 60 ms
  (±20 ms) each way, 4.4 % concealed and still no freezes.

A member without a datagram link, connected over TCP only or reached through another member
acting as bridge, gets the same packets over a stream of the connection there is
(`/fogata/media/1`, each packet after its two-byte length). A stream retransmits and keeps
order, so under loss media arrives later rather than not at all; a short queue (64 packets)
drops packets instead of piling up delay, and a packet that waited more than 150 ms in it is
dropped too, since the other end's jitter buffer has moved on. A device takes at most 2 such streams from each member device and 16 in all, closes one that brings nothing for 40 s (its sender closes one idle for 30), one that stops half way through a packet for 2 s, and one whose sender stops being a member (looked at every 2 s while it is quiet). Received packets pass the same checks as
datagrams: from members only, with the sender's own address as source, and for an
allocation that is for the sender's device. A datagram, a part of a packet or a stream from a
member whose device the local engine has no allocation for (one in no call with this device)
is dropped before it is put together, queued or given a stream's place, so a member of any
group cannot make this device hold its packets. Nothing is routed by the virtual address: a
member that picks a transport identity landing on someone's address (some 16 million
tries, minutes of one processor) still gets only what is sent through its own
allocations, and reaches only those meant for it. With
`FOGATA_MEDIA_STREAMS` set, the bridge of a debug build uses streams even where datagrams
work, for tests.

Not done yet: frame encryption (SFrame style) so a forwarding host cannot decode media. A
bridge that is not in the call never receives media keys.

### 4.3 Calls in rooms

Everyone in a room is in its call; there is no separate "join the call" step. The app
(`app/lib/src/call.dart`) opens one WebRTC connection to every other participant (mesh):

- **Signalling** travels as `MemberRequest::Signal` (offers, answers and small state
  messages, opaque to the node) over the member connection, only between members of the
  room's group. The device with the smaller id makes the offer. Each description carries
  its network candidates, gathered before it is sent (quick: the only server is the
  device's own bridge), so one message per side brings everything and no candidate can be
  lost or reach a later connection. A signal to a member this device is not connected to
  yet is dropped, and members connect to each other at different moments, so the side that
  offers offers again while there is no answer (after 5 s, then up to every 30 s), and the
  other side asks for an offer the same way, the first time as soon as it makes the
  connection's place (a side that kept a connection from before, with a call there that is
  gone, offers only when asked). When the core loses its link with a participant (a
  network change, a short outage) and has it again, a connection with it that is not up
  and whose offer or answer went while the link was down, or was still being agreed when
  it went, is tried again at once (the smaller id offers, the other asks), rather than
  after its tries ran out; one made after the link came back is not, so the two sides'
  tries do not undo each other, and a whole network coming back tries each participant
  once. Only the room's participants get a connection: a signal
  from a member that is not in the room is dropped, and one that arrives before the room
  says its sender is in it, or before this device's call has made its connection with the
  sender, waits for that (the last 4 of each sender): those already in a room offer as soon
  as they hear a device came in, a moment before its call has its media bridge and
  microphone, or is made at all. The node already drops signals that reach a device in
  none of the group's voice rooms, those over 64 KiB (a description takes a few kilobytes)
  and, from each member, those past a burst of 32 and then 8 a second; from a member not in
  this device's room (its join may still be on its way), those past 4 and then one every 5
  seconds, so they never reach the application in numbers. Such a signal also has the
  room asked of its sender (once every 15 s at most), so a join that was lost does not
  wait for the sender's next presence. The
  application drops a description over 64 KiB or with more than 4 media sections, and an
  offer or answer whose sections are not the call's channels (voice, camera, screen, the
  screen's sound, in that order, named 0 to 3): WebRTC makes a channel for every section of
  an offer and decodes what comes on it, drawn or not. An offer or answer (or a screen's
  sound offered or answered on its own connection, which must be one sound section) with
  a sound section without Opus (48 kHz, stereo) or a video section without VP8 among the
  payload types its `m=` line lists is dropped as well: the answer would list the codecs
  it offers, and this device would decode whatever the sender chose among those WebRTC
  has (H.264, VP9, AV1, G.722...). WebRTC ignores an `a=rtpmap` line for a number the
  `m=` line does not list, and takes a listed static type (0 for PCMU, 9 for G.722)
  without one, so a codec named but not listed does not count, and a payload type listed
  twice or named by two `a=rtpmap` lines drops the description; a rejected section (port
  0, not bundle-only) carries nothing and needs neither. An offer from the side that does
  not offer (the device with the larger id) is
  dropped too. It takes only the newest of the offers from a participant still waiting
  their turn, and one a second at most (each makes a new connection or agrees again, and
  with it the bridge may allocate again), adds at most 64 network candidates to a
  connection, and drops a participant's signals but answers while 32 of its steps wait, so
  signal after signal cannot pile up work on its calls. A participant chooses the size of
  the pictures it sends, and WebRTC makes room for frames of that size (up to 16383 by
  16383) before anything is drawn. Every picture of a participant's camera and screen
  reaches its renderer, drawn or not: the renderer tells the size of each new one, and of
  the first too large to draw (over 8K), as it comes, and the statistics read every 2 s
  check what arrives as well. A participant whose camera comes larger than Full HD (1920
  by 1080 either way up, give or take the encoder's rounding) or whose screen comes larger
  than 8K (no side over 8192, at most 7680 by 4320 pixels: exactly what the decoder below
  takes, so no screen it refuses shows black without a word) has its connection closed,
  whatever its state, and gets no
  other while it stays in the call (a connection being made meanwhile is closed as it
  comes); a notice says the connection with them was lost. So does a participant whose
  camera or screen arrives far faster than any copy sends it, in two readings of the
  statistics in a row (`VideoFlood`, `lib/src/call/measuring.dart`): more than 180 frames
  decoded a second (copies send at most 60 on either channel), or more than three times
  what this device's description lets the channel carry (1500 kbit/s for the camera, 20000
  for the screen). A stall does not trip it: WebRTC skips the frames that came late rather
  than decode them all, and a burst of bytes (a key frame, a network catching up) lasts one
  reading. On Linux and Windows, Fogata's
  own libwebrtc (`tools/webrtc`) does not decode such a picture at all: a VP8 keyframe
  whose header names more than 8K (no side over 8192) is refused before its decoder makes
  room for it, and a small stand-in that tells its size reaches the renderer instead, so
  the sender is kept out at its first such picture for no more memory than an ordinary
  call; on macOS the picture is decoded, and its sender kept out, until the macOS
  framework is built the same way. That libwebrtc has only VP8 and Opus built in. No copy
  sends more: a camera larger than that, or a screen shared as
  captured, goes out scaled down to fit (a screen a few thousand pixels under the limit,
  as WebRTC rounds each side of a scaled picture to the nearest pixel; one within the
  limit goes out as it is), and a screen picked while others watch goes out
  only once its first picture tells its size (a screen over 8K, which this device's own
  renderer does not draw either, by the size the renderer tells of the picture it
  dropped; before, such a screen went out unscaled and got its sharer kept out). A
  renderer whose event channel fails stops showing: the call puts a new one in its place
  with the same stream, up to three times for a participant, and then gives the picture up
  (the tile shows the avatar, and a watched screen or enlarged camera is left). A failure
  of a connection the call replaced or closed is not heard.
- **Fixed channels** per connection, in this order: voice, camera, screen, the screen's
  sound. Turning the camera or screen on or off swaps the channel's track (`replaceTrack`).
  A `state` message tells the others what is on, for the muted icon and the tiles.
- **The microphone is open only while it is on.** WebRTC records from the default
  microphone whenever a voice channel may send, even with no track to send, so a muted
  device's voice channel only receives, and its microphone track is closed. Muting or
  unmuting agrees the channels again on the same connection; only the device that made
  the first offer offers again (the other asks it with an `update` message), so two offers
  never cross. (Pausing the channel with `active: false` would avoid the new agreement,
  but flutter_webrtc's desktop code drops changes to encodings.)
- **Video goes in VP8**, the same software codec on every system: each description is
  reordered to put VP8 first before it is set and sent (macOS refuses
  `setCodecPreferences`). Between Linux and macOS, H.264 (in hardware on macOS) sent too
  little for the bitrate estimate ever to grow, and in the other direction showed no
  picture at all. Descriptions keep only VP8 with its retransmission codec and Opus, no
  other codec and no FEC, so fewer decoders ever see what others send; a section of this
  device's own description left with neither (an offer in a codec WebRTC could not match)
  is rejected (port 0, out of the BUNDLE group), so nothing on it is decoded.
- **Voice carries the packet before it** (RED, RFC 2198, over Opus): the voice channel
  also keeps `red/48000/2` whose redundant encodings are Opus, listed first, so each
  voice packet carries the previous one too and a lost packet is made up from the next.
  WebRTC sends the first codec it knows in the other side's description, so a side that
  leaves RED out (an older version) gets plain Opus, and RED only goes to a side that
  listed it and can decode it: between a new and an older version, the older one still
  sends RED when the new one made the offer, and both send plain Opus when the older one
  made it. Opus DTX keeps working (a silent microphone sent 18 packets, 54 bytes, in
  4 s). The screen's sound stays plain Opus. In the lab (two copies, 40 ms ±5 ms each
  way): voice concealed went from 5.1 to 0.2 % at 5 % loss and from 1.3 to 0.3 % at
  10 % (where Opus's own FEC already helps), for 32 to 67 kbit/s per person sent and
  20 to 30 ms more in the jitter buffer (a recovered packet comes one packet late).
- **Voice held up by a stall is dropped.** When the network stalls for seconds, the voice
  sent meanwhile may wait in a queue on the way and then come all at once; WebRTC plays
  all of it, seconds late, and catches up only slowly (in the lab, after a 5 s stall on a
  satellite-like link: up to 1.2 to 4 s late, back to normal 8 to 10 s after the network
  was). Its jitter buffer target is not the cause (it stays at 60 to 100 ms), nor do
  NetEq's field trials for that target help. So after a reading (every 2 s) with nothing
  of someone's voice while their microphone is on, the next five look at how long their
  voice waits beyond what WebRTC aims for; past 400 ms this device stops taking their
  voice for a moment (the voice channel goes send-only, one more agreement each way),
  which empties WebRTC's buffer, and takes it again: 0.3 to 0.5 s of silence, and the
  delay was back to normal 4 s after the network instead of 8 to 10. Readings with
  normal jitter or loss never come near it.
- **Who is talking** comes from WebRTC's audio levels, read every 600 ms (this device's
  own level once, from the first connection). Each is read from the statistics of what
  carries it alone: a speaking participant's voice receiver, and the sender the microphone
  goes out on, a few entries each rather than the whole connection's report; the whole
  report only while those list nothing yet. WebRTC still gathers the whole report inside
  and filters it (kept 50 ms, so the second reading on the same connection costs little):
  what is saved is turning every entry of every connection into a map, and handing it
  over, each time.
- **Each person's connection** shows as bars on their tile, always (the level's word only
  when it is not good), in the room list only when it is not good, and on the own card. While in a call, each device reads every connection's
  statistics every 2 s: the round trip of the chosen candidate pair, the share of voice
  packets lost (over 20 to 50 packets at least, so a quiet voice that sends a few a second
  does not read as a large loss) and whether anything arrived (the connectivity checks keep
  coming when nobody sends). Good is under 150 ms and 2 %, okay under 300 ms and 5 %, poor
  past either, lost when nothing arrived for 3 s; a new level shows after two readings in
  a row, while losing someone and their coming back show at once. A device's own network
  is the best of its connections, named only with two or more others measured (with one,
  a bad connection may be on either side); it goes to the others in a `net` message when
  it changes and when a connection comes up, and older versions ignore it. Someone's tile
  shows the worse of the connection and their own network, and the details say whose side
  the trouble is on. Readings redraw only what shows the quality, never the whole room.
- **A shared screen keeps its resolution** and drops frames when the network falls short,
  so text stays readable, until that leaves it choppy. Each person who watches gets an
  encoder of their own, and the sharer reads each connection's statistics every 2 s: under
  10 frames a second (or half the rate captured) three readings in a row, the screen goes
  to that person a step smaller (1080, then 720, then 540 lines, below its chosen size);
  once a smaller size goes out at 80 % of the captured rate for 10 s, the size above is
  tried again, after twice as long each time that did not last (up to 160 s). A quality
  of 60 frames a second is chosen for motion rather than text, so there a step smaller
  comes under three quarters of the rate (45 frames a second): kept at its size at 25 to
  30 frames, it moved no better than a screen chosen at 30. The first
  10 s are not judged (WebRTC starts slow and finds the connection's rate then), and a
  new share, another quality or someone who stops watching starts over. What the screen
  may use grows with what it sends: 0.1 bit for each pixel of each frame, at least
  6 Mbit/s (the one limit every screen had before) and at most 20 (1080p at 30 frames a
  second gets 6.2 Mbit/s, at 60 12.4), set by the sender on each connection
  (`maxBitrate`). Each side's description allows up to 20 Mbit/s in the screen channel
  (`x-google-max-bitrate`); WebRTC also takes that as the most its bandwidth estimate may
  reach for the whole connection, so with 6 a 1080p screen at 60 could never get more.
  The processor lowers the screen for everyone, a level at a time: the rate halves first
  (60, 30, 15 frames a second, a level that sends the same as the one above being passed
  over), then 720 lines at 15, then 540 at 15, 10 and 5; it goes down when WebRTC says an
  encoder is held back by the processor (`cpu`) and sends well under its level's rate, or
  when the encoders together keep 1.5 cores busy, and back up a level once the one above
  would fit. A single step from 60 straight to 15 once left a screen there: to come back,
  the encoders had to fit in a quarter of what they took. A share starts lower the more
  watch, and goes there at once when someone starts watching (not at the next reading):
  the full screen for one or two, 15 frames a second for three, 720 lines at 15 for four,
  540 lines at 15 from five. In the lab (a 1600x900 screen, a loaded computer), three
  watchers kept 15 frames a second at full size, while five started at 720 lines fell to
  10 and seven to under 10.
  The number watching also sets a ceiling on the size, on top of the quality chosen (never
  more than chosen, nor than the screen is): one or two watchers get the quality chosen,
  three or four at most 1080 lines, five or more at most 720 (each in a 16:9 box, as the
  qualities are). The mesh encodes the screen once per watcher, and each encoder's memory
  and work grow with its pixels: shared to seven, a 1600x900 screen took the sharer about
  1 GB of memory and a core (`VideoEncoderQueue` threads and one allocator arena of 27-40
  MB each), and a screen shared at its own size (1440 lines, 4K) would take two to four
  times as much per encoder. Under the ceiling a level that sends the same as the one
  below it is passed over, so when the processor allows more, a big room gets frames back
  rather than size. The ceiling comes down as soon as more watch, before the new watcher's
  encoder starts, and goes back up only once fewer have watched for 20 s (to what the most
  that watched meanwhile allow), so someone coming and going does not make every encoder
  change size each time. Like the levels it changes only what each connection is told
  (`scaleResolutionDownBy`, `maxBitrate`): no capture again, no new agreement. Someone who
  starts watching is told how the screen goes out now; before, their connection kept what
  it was told when it was made. The quality card shows what each watcher gets as measured;
  when the ceiling or the processor makes it smaller or slower for everyone, it says so
  apart from a connection that falls short, and no longer says that the others get what
  was chosen. Measured in the lab with a moving 1600x900 screen: at
  600 kbit/s, keeping the size gave 2.5 frames a second, WebRTC's own balance 600x336 at
  15, and the steps 960x540 at 10; at 1.5 Mbit/s, 11 frames a second at full size (the
  balance: 800x450 at 26); on a fast link full size at 26 (the balance shrank it to
  1200x675 even there, so it is not used). The camera is asked for at 1280x720 and 30 fps,
  in plain numbers, which macOS needs.
- **No camera, no picture.** Turning the camera on where the computer has none (Linux and
  Windows answer with a stream without a picture, macOS with `NotFoundError`) says that no
  camera is connected and leaves it off. A tile shows the avatar until its camera or screen
  sends its first frame.
- **A camera turned on in a call starts sharp.** While only voice flows, WebRTC's bitrate
  estimate often stays at its start (a muted or quiet microphone sends next to nothing, and
  nothing can probe for more before a picture has gone out). The camera's channel stops
  asking for bitrate 2 s after a connection starts without a picture, keeping the share it
  had then, about half the estimate (the screen's channel has the rest), and its first
  pictures are judged against that share: under 500 kbit/s WebRTC drops them, making the
  picture smaller each time, down to 320x180 at 10 frames a second, and grows it back only
  a step every 8 to 10 s. With WebRTC's own start of 300 kbit/s, two copies on one PC,
  both muted, got 320x180 at first and 1280x720 after 60 s (960x540 after 30 s); with
  three, 360x203 and the 540 lines of their step after 39 s; with six or eight (360 lines),
  160x90 and 360 lines after 25 to 29 s. So each side's description has the camera's
  estimate start at 1200 kbit/s (`x-google-start-bitrate`, taken once, from the first
  description): the same copies then got 1280x720 (two) or their step's size (three, six,
  eight) from the first picture, 1.1 to 1.7 s after the camera was turned on, and the
  step's bitrate within 1 to 3 s. Where voice did let the estimate learn the connection
  (in the network lab), nothing changes: a slower connection still starts the camera
  smaller (640x360 at 600 kbit/s, 320x180 at 300). On a 600 kbit/s link the estimate
  knew nothing of (muted copies), the camera went 1280x720, then 960x540 at 30 fps within
  a second, with no packet lost and no freeze (before: 320x180 at 10 fps, 640x360 at 15
  after 30 s); with 5 % loss and 40 ms each way, 720p at 1.6 Mbit/s as before.
  A participant's description is taken without any lowest bitrate (`x-google-min-bitrate`,
  which no copy sends: WebRTC would keep that connection's estimate above it however
  congested this device's network, and every voice it sends would suffer) and with a start
  of at most 2400 kbit/s, twice what copies ask (`peerBitrates` in `call/sdp.dart`).
- **A camera goes out smaller the more people are in the call.** Everyone sends their
  camera to everyone else, with an encoder for each, so eight people with cameras would
  each upload up to 7 x 1.5 Mbit/s and encode seven 720p pictures. Instead, by the number
  of people in the room (this device included), the sender sets on each connection's
  camera channel the most it sends (`maxBitrate`), the most of the picture's height it
  keeps (`scaleResolutionDownBy`) and the most frames a second (`maxFramerate`):

  | People | Picture | Each connection | Upload in all |
  |--------|---------|-----------------|---------------|
  | 2 | as the camera gives it | 1500 kbit/s (the receiver's limit) | 1.5 Mbit/s |
  | 3-4 | 540 lines | 900 kbit/s, 30 fps | 1.8-2.7 Mbit/s |
  | 5-6 | 360 lines | 500 kbit/s, 30 fps | 2.0-2.5 Mbit/s |
  | 7-8 | 360 lines | 350 kbit/s, 20 fps | 2.1-2.45 Mbit/s |

  With 2 people the camera goes out as before. Each bitrate is at least what WebRTC's VP8
  wants to start a picture of that size (1500 kbit/s for 720p, 800 for 540p, 500 for
  360p); at 350 the last step sends as many bits a frame as the one before, at 20 frames,
  and WebRTC goes down to 480x270 when it must. Measured with a made-up camera of 1280x720
  at 30 fps (tests), each step alone on one connection between two copies on a PC (Core
  i9-9900K): 720p took its 1500 kbit/s at 30 fps with 5 to 6 ms of encoding a frame (15 to
  17 % of a core), 540p 900 kbit/s at 26-30 fps (10-14 %), 360p 500 kbit/s at 30 fps (8-11
  %), and 360p at 20 fps 350 kbit/s (5-9 %): with eight people the encoders take about
  half of what seven 720p pictures took, and the receivers decode smaller pictures. A
  new number of people applies at once, as new parameters on the senders: nothing is
  agreed again and the picture does not stop. The 1500 kbit/s each receiver's description
  allows stays the upper bound. A shared screen is not affected: it goes only to those who
  watch it and adapts to each (above). In calls of copies on that one PC, every copy with
  its camera on, half of them leaving and coming back, a minute each time: with four, each
  copy sent a median of 2.2 to 2.6 Mbit/s (before: 2.5 to 3.9, peaks of 5.7) with 0 to 3
  freezes seen (before: 3 to 9); with six, 2.4 to 2.5 Mbit/s (before: 3.2 to 5.8, peaks
  of 8.8) and no freezes (before: 24 to 30 each). The copies stepped within a second each
  way, and two left alone went back to 720p at 1500 kbit/s at once. Six copies still
  overload one PC (drawing every picture in software takes most of it), so frame rates
  there say little.
- **One slow connection does not make the camera smaller for the others.** Every
  connection has an encoder of its own, all fed by the one camera. Left to itself (the
  `balanced` degradation preference), an encoder that falls short of bandwidth or
  processor asks the camera for smaller pictures or fewer frames, and the camera gives
  every encoder the smallest size and lowest rate any of them asked for: in the lab,
  three copies (540 lines), one of them on a 300 kbit/s link for 70 s, the third got
  720x405 within 10 s and 480x270 at 15 frames a second within 20 s, and 960x540 at 30
  again only 44 s after the link was back. So the camera's channel has WebRTC change
  neither size nor rate (`maintain-framerate-and-resolution`, libwebrtc's `DISABLED`; our
  copy of flutter_webrtc had to be taught the name on Linux and Windows), its most frames
  a second is the same on every connection (the camera's rate is the lowest any encoder
  asks for), and the app sizes the picture for each connection itself, the way a server
  would pick a simulcast layer for each receiver: every 2 s it reads what the camera's
  encoder on that connection is given (`targetBitrate`, at most the step's bitrate; else
  the path's `availableOutgoingBitrate`). At 75 % of the step's bitrate or more the
  picture goes out at the step's size; at 40 % 1.5 times smaller, at 22 % twice smaller,
  below that three times smaller (`scaleResolutionDownBy` on that connection only; its
  `maxBitrate` stays the step's, so the estimate can grow back). Two low readings in a row
  make it smaller at once; it grows back once every reading for 10 s wanted a larger
  picture, as large as the least of them wants, after twice as long each time a larger
  one did not last 20 s (up to 80 s). The first 6 s after the camera starts going out on
  a connection are not judged, while its share grows to its rate. Each change shows in
  the call log (`camera to <peer>: ... lines`). The same lab run with this: the third
  copy got 960x540 at 30 frames a second the whole time; the slow one 320x180 at 30 (100
  to 160 kbit/s) from 4 s after its link slowed (before: 480x270 at 15, 130 to 160), and
  back to 960x540 48 s after its link was back, in three steps that followed the
  estimate's growth. With two copies and the same slow link, the slow one got 427x240 at
  30 (before: 480x270 at 14 to 15) and 1280x720 again 51 s after (before: it stayed at
  960x540 to the end, 60 s after). Without a slow link nothing changes: 1280x720 at 30 and
  1500 kbit/s with two, 960x540 at 30 and 900 with three. The slow copy's picture froze
  about as long as before while its link slowed down (4.3 to 7.3 s in all, before 5.2 to
  8.7). The processor no longer lowers the camera either: the steps by number of people
  bound what the encoders do (above), and with three copies on one PC WebRTC's own check
  of the processor measured the encoders at 17 to 21 %, where it lowers them past 85 %.
- **Bandwidth estimate on lossy links**: WebRTC's newer loss-based estimate
  (`WebRTC-Bwe-LossBasedBweV2`) holds the estimate and stops probing for more while
  packets are lost at random, as on Wi-Fi or a satellite link, so a shared screen could sit
  at a few hundred kbit/s on a link with far more room. It is turned off in our copy of
  flutter_webrtc (field trials, all platforms); the older loss-based estimate lowers the
  estimate only past 10 % loss, and the delay-based one still finds the limit of a
  congested link. In the lab, on a satellite-like link (one-way delay stepping between 20
  and 60 ms every 15 s, bursts of jitter, 2 % loss) with a moving screen, the estimate's
  median went from 1.5 to 5.3 Mbit/s and frames under 5 a second from 7 readings in 44 to
  none; on a congested 600 kbit/s link, with or without 1 % loss, nothing changed (about
  460 kbit/s, voice concealment 0.1-0.3 %). For tests on Linux and Windows,
  `FOGATA_WEBRTC_LOG=info` puts libwebrtc's own log (estimate, probes, route changes) on
  standard error, and `FOGATA_WEBRTC_FIELD_TRIALS` replaces the field trials (debug builds).
- **Stream quality**: the person sharing chooses its resolution (720p, 1080p, or the
  screen's own) and frame rate (15, 30 or 60), from beside the share button or over
  their own screen; the choice is kept in `app.json`. It applies at once: the sender
  scales the picture down to fit the chosen 16:9 box (`scaleResolutionDownBy`), and the
  screen is captured again at the chosen rate, since WebRTC's encoder does not keep to
  a lower `maxFramerate` by itself. Under Wayland the system's dialog cannot be shown
  again for that, so the screen is captured at up to 60 frames a second and the capture
  lets only the chosen rate through. Only the values set go back to the sender: the
  parameters WebRTC hands out carry empty ones (no scalability mode, a bitrate of 0) that
  it refuses, and with them the whole change. The choice is the most anyone gets: under
  it, the card lists each person watching with the size and rate their picture goes out
  at, from the readings every 2 s, and says when someone's connection made it smaller
  (see the steps above), so a choice that seems to change nothing for them is explained.
  "Source" shows the screen's own height ("Source (1080p)").
- **The sharer's own share shows as a card, not its picture.** Drawn live in its tile,
  the picture showed inside the window shared whenever that was Fogata's (or the screen
  under it), was captured again with it, and so on without end: every frame changed, and
  the share went out at a few frames a second. The card says the screen is shared and how
  many watch, with a button that shows the picture enlarged and a note that doing so
  while sharing Fogata itself makes that mirror. Fogata's own windows are also left out of
  what can be shared: on macOS out of the list (by window number) and out of a shared
  display (ScreenCaptureKit leaves out the application, windows opened later too), on
  Windows out of the list by the process that owns each window, and under X11 by
  Fogata's window title. Under Wayland the system's dialog offers every window and
  nothing can be left out, so the card is what keeps the mirror away there.
- A connection that fails, or that does not come up within 15 s, is tried again after a
  pause of 4 to 6 s while both are in the room. Each offer carries a number that its
  answer sends back, and an answer to an offer replaced since is dropped: when both sides
  tried again at once, the answer to the first offer used to be set on the connection
  made for the second, which then waited minutes for the other side's closed one (older
  copies send no number and are answered as before). Someone whose screen was watched,
  or whose camera filled the room, and who leaves the room and comes back within 3
  minutes still sharing it (their network was gone a while) is shown again, unless
  something else is shown meanwhile. A call comes back only once the member connection
  does: signals to a member that is not connected are dropped.
- Devices: the microphone, speakers and camera can be chosen in Settings or from the room,
  also in the middle of a call. WebRTC's voice engine starts with the first connection,
  picking the system's default devices, and would stop with the last one, taking the list
  of microphones and speakers with it (after a microphone test or a call, the settings
  listed none); our copy of flutter_webrtc keeps an idle connection of its own from the
  first connection on, so the engine runs until the application quits. A call or a
  microphone test therefore chooses its devices each time, the system's default by its own
  id too: the engine keeps those it was given last. WebRTC keeps the chosen microphone only
  as its place in the list of inputs and, on Linux, reads the list again each time it
  starts recording (when someone comes into a room where this device was alone, say); our
  copy of flutter_webrtc selects it again at its new place whenever an input comes or
  goes, so it never becomes another microphone or the default one.
- Noise suppression (Settings, under the microphone; off by default, kept in `app.json`)
  is RNNoise (`crates/fogata-denoise`, nnnoiseless: a small neural network that tells
  speech from noise in 22 bands and lowers the bands that hold noise, so a fan, a hum,
  typing or traffic fade while the voice keeps its level). Our copy of flutter_webrtc runs
  it on WebRTC's audio thread as the audio processing module's capture post-processing, on
  every 10 ms of the microphone's sound after echo cancelling and gain control
  (`fogataNoiseSuppression`, see `app/third_party/flutter_webrtc/FOGATA.md`). The switch
  only flips a flag that processing reads: it applies from the next 10 ms, also in the
  middle of a call, and the microphone stays open (the old switch, WebRTC's own
  suppressor, had to open the microphone again: 120-150 ms here, 1-2 s on a Mac). It adds
  10 ms of delay while on (RNNoise answers each 10 ms one frame later; at 16 or 32 kHz,
  resampled to 48 kHz and back, about 2 ms more) and none while off; each change
  cross-fades over 10 ms. The microphone is always opened with echo cancelling and gain
  control on and WebRTC's own noise suppressor off, on every system (macOS reads only
  WebRTC's own names, `goog...` in `mandatory`, and keeps whatever was set last for a name
  left out, so all three are named). Both suppressors at once were measured too: 8 dB less
  steady noise than RNNoise alone, but the clean voice 1 dB quieter and 2 dB thinner
  around 500 Hz (RNNoise alone: 0.2 dB), and "off" would no longer be off. Measured on
  Linux with real recorded speech and made-up noise, 20 s off and on in turn (level
  between words; voice level while speaking): steady fan, hiss and hum about 30 dB under
  the voice: -47 to -62 dBFS (WebRTC's own: -46 to -62), voice 1 dB lower; the same noise
  12 dB louder: -32 to -62 dBFS (WebRTC's own: to -49); noise alone: -41 to -73 dBFS
  (WebRTC's own: to -58), often silence that Opus then does not send; typing: 5.6 dB lower
  (WebRTC's own: 1.4 dB); clean speech: 0.2 dB lower, the octave around 8 kHz 3 dB lower.
  RNNoise takes about 60 µs of every 10 ms at 48 kHz, 0.6 % of one core here, and the same
  on WebRTC's audio thread in a real call (WebRTC's own suppressor: 0.1 %).
- Each person's voice can play louder or quieter on this device only, from 0 to 200 %, or
  not at all ("Mute for me"): the gain is set on the received audio track, nothing changes
  for the others, and the choice is kept per person in `app.json` for the next calls. The
  level that lights a tile is measured after that gain, so someone muted here shows no
  talking.
- A shared screen is sent only to those who watch it: the others see a LIVE card with
  Watch, and a watched screen fills the room in its own shape (a camera does too, with a
  click), with the others in a strip right under it that scrolls sideways when they do
  not fit; their tiles keep the grid's look, the picture in the middle (smaller) and the
  name at the bottom left. Without one, the tiles take the size that fits best, and below
  160 by 100 they keep that size and the grid scrolls. A room takes 8 people at most, each can share
  a screen and a camera, and one screen is watched at a time. Alone in the room, its own
  shared screen and camera show on its page as they will with others; nothing is sent
  until someone comes in.
  Watching only puts the screen on the watcher's connection (`replaceTrack`; the channel
  was agreed with the connection), and the watcher's tile waits for the first picture's
  size. The portal's capture on Linux and ScreenCaptureKit on macOS give a picture only
  when the screen changes, so the sharer has the capture give its last picture again when
  someone starts watching (the fork's `fogataScreenRefresh`). Both sides also follow it at
  each reading of the statistics: the sharer sends the screen again (its last picture, then
  the track put on again, as closing and opening it does) to a watcher whose encoder made
  nothing since they started watching, three times at most; the watcher shows a screen
  that has no picture yet again (its renderer starts over) and tells the sharer again that
  it watches it, at the second, fourth and eighth readings, and never after.
  Its controls (who, how many watch, the connection's quality, sound, full screen and the
  way back) show while the pointer moves over it and hide after 2.5 s. The quality comes from
  the WebRTC statistics every 2 s: round-trip time and packets lost since the last reading
  (good under 150 ms and 2 %, poor past 400 ms or 8 %), with the picture's size and rate.
  A fourth channel carries the screen's sound, also only to watchers, with its own volume
  per person, in two channels (Opus `stereo=1;sprop-stereo=1` in that section only, and
  in the section of a Mac's connection for it; the voice stays in one): WebRTC encodes two
  only towards a side whose description asks for them, so a version before it is sent
  one, mixed down by the sender's WebRTC, and takes the descriptions as before. Linux and
  Windows capture it in two; a Mac's capture mixes it into one. A screen's sound plays only while this device watches that screen, and its
  connection of its own (macOS, below) is refused otherwise, whoever sends it. On Linux it is every other application's sound, linked in PipeWire,
  without Fogata's own, so nobody hears the call back: an output is taken only once its
  process or executable is known, and one found later to be Fogata's own is unlinked.
  Without PipeWire (PulseAudio itself the sound server) a shared screen has no sound,
  and the sound button does not show: what PulseAudio's output plays includes the call.
  Instead, while a screen is shared, a sound button that cannot be pressed says why
  (`fogataScreenSoundServer` asks whether PipeWire takes a connection, once a share came
  without sound; a shared window, which has none for another reason, does not ask). WebRTC opens the microphone for
  any channel of sound that sends, so with the microphone off that channel only sends
  while WebRTC records from "Fogata silence", an input PipeWire runs for the app (a null
  sink shown as a source) that only gives silence; elsewhere it waits for the
  microphone. Each Fogata running makes its own, named after its process ("Fogata
  silence (<pid>)", node `fogata-silence-<pid>`), and picks only its own: another one's
  goes away with that process, and the recording would move to the default microphone.
  Before each share it checks that its own is still there. The app goes back to the microphone only when it is turned on or the call
  ends: a channel may keep sending until the other side agrees again, choosing another
  input while WebRTC records opens it at once, and a recording whose input goes away
  moves to the default microphone by itself. Choosing the silent input and going back
  to the microphone take turns, so a call that ends (or a microphone turned on) while
  the silent input is still being chosen goes back to the microphone once it is: WebRTC
  keeps the input it was last given from one call to the next. For the same reason each
  call on Linux chooses its input at the start even with "System default" (the first
  real input listed, never a silent one). The
  voice never reaches the screen's channel: libwebrtc hands what it records only to the
  microphone's track. On Linux a shared window goes without sound, and the sound button
  does not show: the portal does not say which application a window belongs to, and
  every other application's sound would go with it. The app does not ask for sound with
  a window picked from its own list (X11), and under Wayland the portal's answer says
  whether a window or a screen was chosen (a portal too old to say, offering windows,
  counts as a window). On Windows it is
  every other application's sound for a screen, and only the shared window's program
  (with the processes it started) for a window, or none when that program cannot be
  found, and with the microphone off it waits for it. On both, the sound starts only once
  the picture's capture is made, stops if that capture fails, and goes with the capture it
  was started for: a screen captured again at another rate keeps its sound when the old
  capture ends. On both, the
  screen's sound source is made with the microphone's echo cancelling, gain control and
  noise suppression settings: it never goes through them, but WebRTC sets its one audio
  processing module from each source whose channel starts sending, and a source made
  with them off had turned them off for the microphone while the screen's sound went
  out. macOS's WebRTC
  hands what it records to every channel of sound, the voice's too, so a Mac sends its
  screen's sound (every application's but Fogata's, taken with ScreenCaptureKit; for a
  shared window only its application's and that application's helpers') on a
  connection of its own to each watcher, made by a second WebRTC whose audio module
  renders by itself without any device: it never opens the microphone. The sharer always
  offers that connection (`sound`, `sound-answer`, `sound-end` among the call's signals),
  and closes it when the watcher stops watching. A Mac shares a window through
  ScreenCaptureKit too, at the full frame rate (WebRTC's own window capture gave about 5
  pictures a second).
  The share stops by itself when the system ends the capture: the system's own "stop
  sharing", the shared window closed, or (Linux) the stream from the portal failing. A
  capture that does not start (the screen recording permission refused on macOS) is a
  failed share, never a screen that stays still.

A device that starts again comes back under a new transport identity, while its old
session may linger at the others until its connections time out (a crash leaves no
goodbye). When the new session proves its membership, the old one is forgotten and its
connections closed, so messages for the device, call signals among them, reach the new
one at once. (Before, a call after a restart could take as long as the old connections
lingered.)

One identity is meant to run on one computer: a second copy cannot open the same data folder
(`running.lock`), but a folder copied to another computer can run there at the same time
(only one whose data is plain: a sealed folder needs its key from the keychain of the
computer that sealed it, 5c).
The others then see one device key with two live sessions and reach one or the other, so
both copies are told ("This identity is open on another computer too. Close Fogata there:
one identity works on one computer at a time."), at most once in ten minutes, in a toast and
in the bell until dismissed there; nothing else changes. Since the transport identity is
new at every start, two sessions of one key that are alive at once can only be two copies,
and a restart's old session is never alive:
- the copies may meet: a membership proof naming this device's own key comes from a live
  copy (it is fresh and bound to both transport identities);
- a member or friend that sees a new session of a device while an older one is still
  connected asks the older one whether it is there (`OtherSession`); a member's older
  session is let go after the answer, or after none. A restart's old session cannot answer; a live copy does, and then the newer session is
  told too. Only sessions that proved to be the device are told, so nobody learns anything
  about a device it could not already see. Older devices cannot read `OtherSession`, which
  only fails that request: a copy of an older version is never told, an updated copy
  beside it only when they meet or when it is the older session, and older members and
  friends tell nobody.

A member or friend could make up `OtherSession` and show the notice; it changes nothing
else.

Datagrams need QUIC, so members prefer it: devices found on the local network are dialled
on their QUIC addresses only, and once a member is verified or identified over another
transport (TCP), its QUIC addresses are dialled once more to get a datagram link. Identify
shares only confirmed public addresses, and a home router rarely sends traffic from inside
back in to its own public address, so two members on one network that did not find each
other there (another subnet, a guest network, a virtual machine behind NAT) could not get a
datagram link. Each device therefore tells every member it verifies its own local addresses
(`MemberRequest::Addresses`, QUIC first; never to anyone else), and the QUIC dial for media
tries those first. Only local addresses among them are kept, dialled for media only on a
network this device is on (the same private IPv4 /24 or unique-local IPv6 /48, or
carrier-grade NAT when this device is on it too), and new ones at most once a minute: a
member could otherwise make the device send packets to any address, its own networks' and
VPNs' included, again and again. mDNS answers may likewise name carrier-grade NAT and
unique-local addresses only of networks this device is on. The
local TURN endpoint accepts its random username and password only. It listens on
`127.0.0.1`, and there the source address is not checked, because libwebrtc sends from the
computer's own address even to `127.0.0.1`. On Windows a socket bound to a network address
(as libwebrtc binds its sockets) cannot send to `127.0.0.1` at all, so there the endpoint
listens on the address of the interface toward the internet (found from the route to a
documentation address, without sending anything) and drops every packet that does not
come from that same address: other devices on the network get no answer. On every system,
media (channel data, Send indications) goes on to the TURN server only from an address and
port the server has answered a request with valid credentials from in the last 15 minutes
(allocations and permissions are refreshed more often), and at most 5000 packets a second
from each: a device on the network that fakes this computer's address can reach nothing
without guessing one of the engine's ports, and then no more than the engine could send.

Voice and video are encrypted end to end between the WebRTC engines (DTLS-SRTP) and again
by QUIC on the way.

In field tests between a Linux desktop on a home connection and a Mac on mobile data
(`call_test.dart`, section 8), friends and a call were set up within about a minute and a
half, with round trips of 40 to 80 ms, voice with few or no lost packets and 10 to 20 ms of
jitter, and a 1600x900 screen shared at 27-28 fps with no losses or freezes. Three devices
on one network (Linux, macOS and a Windows virtual machine behind two NATs) connected to
each other within about 10 s and shared their screens at once; the virtual machine reached
the Mac once a coordinated QUIC connection, punched through the NATs with another member's
help, replaced the first one that member relayed. Members share their local addresses, so
two devices whose member connection runs over TCP still find a QUIC path for media; media
also works forced over streams (`FOGATA_MEDIA_STREAMS`). A test of six real devices, each on
its own computer, remains to be done.

### 4.4 Realistic limits

| Mode | Each participant uploads | Host uploads | Realistic size |
|------|--------------------------|--------------|----------------|
| Voice, mesh | (N-1) x ~32 kbit/s | - | up to ~8 |
| Voice, host | ~32 kbit/s | (N-1)^2 x ~32 kbit/s | ~20 |
| Camera, mesh | 1.5 Mbit/s with 2; then (N-1) x 0.9, 0.5 or 0.35 Mbit/s (4.3), 2.7 at most | - | 8 |
| Video, host with simulcast | one stream in 2-3 qualities | depends on the host's line | 8-10 on symmetric fibre |

## 5. Group management (fogata-state)

### 5.1 The group log

A group is a log of signed entries that every member keeps; the current state (name, members
and their order, who manages, key epoch) is computed from it. Messages and files are never
in it.

```
Entry { group_id, parents: [hash], author: device key, time_ms, body, signature }
```

`group_id` is random and fixed at creation. `parents` are the latest entries the author knew,
so entries form a graph. Every member orders the graph the same way (an entry after its
parents; among entries ready at once, those of the members that came in last first, so that
of two changes made apart the one of the member that came in first applies last and stands;
then by entry hash) and applies each entry if its author was allowed to make it at that
point; others are ignored. When a device came in is counted in the order itself, from the
`Create` and `Add` entries before it and, for a `Join`, from the first entry signed on it
if that is a `Welcome` naming its device (the one that lets it in), without judging rights (a device not in yet counts as the
newest, one that comes back counts from then): it only breaks ties. Members exchange missing entries whenever they connect.
An entry is at most 64 KiB and names at most 64 parents. At most 256 entries (4 MiB) wait
for parents that have not arrived, at most 64 of them (1 MiB) from one author; past that,
those of the author with the most waiting are dropped, the last to arrive first (not by
their own time, which the author chooses), so one member cannot push out the entries of the
others. Which entries wait only delays them, so devices compare the heads of their common
order, waiting entries left out, to tell whether they need to sync. Logs are stored and sent
in the common order (spare entries, below, after the rest) and read back as one batch, so a
long log never loses entries to that limit when it is read.

What costs little is checked before an entry's signature: its group, whether the log already
has it, its length and number of parents, and whether the log named its author (its creator,
a device added, or one that joined with an invite of a named device). Entries of a device
never named, or sent again, cost a device no signature check.

A device keeps an entry only if its author could make it in its own view of the log: the
entry applies on top of the group that its parents, theirs and so on make, as the last entry
of that log (`GroupLog::insert`). Beyond what applying checks, a box in an `Add`, `Welcome`,
`Remove` or `Rotate` must hold one epoch secret (48 bytes, one per device), a removal's
new epoch must be sealed to members only, and the entry must fit in its author's run
(below). Commits and Resyncs are held to what the MLS chain
judges on the group their entry saw (5.5: the author a member, a first Commit by a manager,
adds by managers, of members, removals of devices no longer members by someone allowed, the
Commit built on signed before it, with the epoch after its own); a KeyPackage needs its
author to be a member; an `MlsJoin` is judged as a join. An entry of a kind this version
does not know is not judged. The answer depends only on the entry and the entries before
it, which its hash fixes, so every device comes to the same one, whatever else it has and in
whatever order entries come: an entry whose parents are not all here and judged waits for
them (within the waiting limits above) and is judged once they are, and one after an entry
refused waits for good, as after a parent that never comes. A refused entry is not kept and
not passed on, and `append` refuses to write one. A log read from disk is judged the same
way (once: see below), so an entry an earlier version kept and this rule refuses goes when
the log is read, on every device alike. So a member that does not manage cannot make anyone keep a rename, a
picture, an invite, a key, a room or a rule, nor any move or removal of someone it is not
above; and a large entry is large only where its kind allows (a picture of 48 KiB, a
rotation of about 140 bytes per member, a KeyPackage of 2 KiB).

Working out the group the parents make costs as much as working out the whole log, so it is
done only when needed. New entries are signed on every entry of the order (at most 64 at
once, the rest go in the next entry), whether or not it applied, but those past their
author's run (below) and those of a kind that never applies here (kinds this version does
not know, the MLS kinds other than Commits and Resyncs). Those change nothing: they never
apply, bring nobody in nor take anybody out, and count in no run but that of an entry signed
on them (below). An entry signed on all the others
is judged on the group as the log has it, since only those are left out of what is before
it; any other is judged on the group its parents make, worked out for it and kept for its
set of parents (64 sets at most), which entries written at once share. A member can
therefore make each device work the group out once for every entry it signs on parents of
its choosing, at the rate its budget allows (3.6); and of a message from another device a
node works it out for at most 8 entries (`GroupLog::insert_some`), leaving the rest out as
if they had not come, for the next sync to bring. That counts the entries that came before
their parents and are judged once the parents are here as well: a member sending a whole
sync answer of entries ahead of the parent they share, each also signed on another old
entry, held a node for 10.8 s with 1 000 of them in a log of 4 096 entries, and holds it
for 0.17 s (measured in release). Each entry judged and refused also costs its author 64
entries of its budget (3.6), which may go a burst below nothing, and an answer that led to
a refusal is not followed at once by the next sync. The author pays, not whoever sent it:
an honest member passes on entries that wait for their parents there without judging them,
and must not pay for what another wrote. The author pays whatever batch let the entry be
judged, so one that sends its entries ahead of their parent, then the parent, alone or
through another member, pays for each once it comes. Only an author that ever sent entries
here has a budget to charge, and none is made for one that did not (a key made up for a
join, say): a join refused costs no more to judge than one taken. An author sending nothing
but refused entries gets about one judged a minute once its burst is spent; one whose
refused entries only others pass on is held back by their budgets, at one entry each, which
is what any entry on old parents costs. 4 096 entries signed on old parents cost a
node about 0.2 s on a two-year log of 20 members (8 s when each was kept, as before), and
512 cost 0.4 s on a log of 2 000 entries with a thousand removals and leaves.
Some logs are taken whole, with no such bound: a group a friend offers (4 096 entries at
most), a direct chat's group a friend offers (64), a removal of this device shown by a member
it no longer verifies (512 it lacks) and the log a manager lets a joining device in with. Two
members each signing only on its own last entry make every entry cost a group worked out:
4 096 of them take 12 to 13 s to judge in release. The node judges these on a blocking
thread instead (`fogata-net/src/node/judging.rs`), two at once and eight waiting at most (two
of one peer, 16 MiB), each on a log of its own or a copy of the group's, after the checks that
cost little (bounds, the sender's budget, the removal's signature) on its loop. What they
bring is taken on the loop once judged, as if it came then: a group that runs here by now
takes the entries as any batch from a member, and a removal or a joining device's log is
taken whole by putting the judged copy in place of the log when nothing came meanwhile that
the copy lacks (or as any batch if something did). A removal of 512 such entries in a log of
3 584 costs the loop 3 ms before and 1 ms after, and 2 s on the thread. A job past those
bounds is refused, as an offer past the sender's budget is. A newer job of one peer for the
same group and of the same kind takes the place of its own waiting; another peer's never
does, so a bare removal shown by anyone cannot push out the whole one a member showed.

A device reads its logs back at every start, and a member may bring a group to 4 096 such
entries, eight a sync at a time: judged whole from the file, they took 4.4 to 8 s in release at
every start (0.17 s for an honest log of that size). So what was judged of a log is kept beside
it (`logs/<group>.log.judged`, and `group.log.judged` of a direct chat), each time the log is
written (`GroupLog::verdicts`): a digest of the entries, the level and way of judging of the
code that judged them, and which of them were refused and which wait for parents. Read back
with the very same entries by code that judges the same way, the rest are taken as judged
(`GroupLog::read_judged`) in 24 to 37 ms; otherwise (a log changed, cut short or written
before these were kept, another way of judging) the entries are judged whole and what comes of
it is kept for the next time. Whoever can write them can make the device keep what it refused,
so in a sealed folder they count only sealed with the folder's key, for that log (signatures
are not checked again then; in a plain folder they are, in 0.16 s, and the folder holds the
identity in the clear anyway). They are not moved with the account (5e), and are judged again
on the new computer. A group a friend offers again, which the node judged away from its loop,
is taken into the copy kept here without judging again what the node judged
(`GroupLog::insert_judged`): only entries that waited there for parents this copy has are
judged, at most 8 needing the group worked out for their own parents per offer, as from any
member, since this runs on the core's event task with the store held (256 of them, as many as
may wait, would hold both for up to 7 s). The others are left out of the copy kept here and
come with the next offers (a friend offers the group again whenever it changes there, and at
each new connection), or with syncs once the group runs; what the node judged of the offer, which shows this device and the friend as
members, is always taken whole. A removal undone (below) runs the group again without asking
only when the offer was taken whole; otherwise the person is asked, since an entry left out
could have let this device in another way.

Every member keeps what it takes, so how much a member may make the others keep is bounded
by what the others write (`fogata-state/src/budget.rs`). A device's run is the entries it
wrote that no entry of another member is signed on yet, directly or through other entries;
an entry of another member takes out of it the entries it is signed on, and no others, so
an entry written long ago takes out nothing written since. A run holds at most 4 096 entries
that count and 4 MiB of them, what a node takes from one device at once (3.6). Every kind
counts but the `Create`, the kinds that never apply here and `Join`s: renames, icons,
pictures, invites and their cancelling, limits, who manages, rooms, room rules, moves up and
down, adds, removals, leaves, `Welcome`s, rotations, and MLS Commits and Resyncs, whether or
not they apply. A `Join` is part of no run, like the kinds that never apply: the `Welcome`
signed on it brings it in and it counts there (below).

Bringing a device in and taking it out count in the run of the member that brought it in:
the author of its `Add`, or the manager whose `Welcome` let it in with its `Join` (the
join counts with that `Welcome`, as an entry of no run signed on; not in the run of the
device that joins, nor in that of the ticket's issuer, whom anyone holding the ticket could
otherwise charge); and, in that same run, its removal or leave (as the order counts who came
in when, without judging rights; a removal or leave of the creator counts nowhere). Removing a member or
leaving is never held back: a member cannot keep itself in by writing, so a removal or
leave may fill a run past its room, and that member then brings nobody in until its run has
room again. These entries leave a run only through an entry of a device that came in
before them: a device never takes out of a run the entry that brought it in, nor those of
devices that came in after it. So a member cannot bring devices of its own in and out
without end; and a join or leave, which honest devices sign on what everyone signs on, still
takes out of the others' runs everything else it is signed on.

Entries that are part of no run, those past one, those of a kind that never applies here and
joins nobody let in, are spare (below): nothing is signed on them, and devices let them go. An entry signed on
some anyway, directly or through others like them, makes every member keep them, so they
count with it in its author's run, entries and bytes, all those that no entry of a run
placed before it had brought in; and it is past its run if they do not fit, a removal, leave
or join too. Honest devices never sign on them, so this holds back only a device that writes,
say, a chain of entries of a kind nobody knows and signs a leave or a rename on the last.

Whether an entry is past its author's run is worked out from the common order alone, as
each entry is placed, never from what applied, so the rounds of the losses change nothing
of it. It is checked like the rest, as the entry's author saw the log: an entry past its run
there is refused, not kept nor passed on, and those after it wait for good, as after any
entry refused. Entries signed apart, none seeing the others, each fit in the run where it
is judged; in the common order only those that fit apply. Those past it apply to nothing
and are signed on by nobody, and neither is an entry signed on one of its own author's past
its run, so they are spare entries (below) and go. In random histories with devices that
write six entries in a run, every copy keeps and works out the same, in whatever order
entries come.

So a device makes every member keep at most 4 MiB (4 096 entries) that count for each entry
of another member signed on what it wrote, plus its removals and leaves that fill its run
past its room (one for each device in that it brought in), plus its spare entries. A
manager bringing devices of its own in and out with tickets and adds, as fast as it can,
stops once the comings in and goings out fill its run (a join and a removal each time),
until a member that was in before them signs on them; only a device of its own that stays
in, as with two devices that sign on each other's entries (below), lets it go on. A manager renaming 100 000 times, as fast as a client that ignores every
refusal can, leaves 4 093 renames on a group of 20 members two years old (1 MB, read back in
0.2 s; 17 MB and 4 s before); one that signs 10 000 renames on older entries leaves 4 096 of
them and the 64 spare a device keeps (1 MB read back in 5 s; 2 MB and 15 s before), though
taking the 10 000 at once costs a node 41 s rather than 29 s (measured in release on a
16-core PC; of a message a node judges at most 64 such entries, as above). Honest devices stay far below: a rotation needed because someone left, a key that does
not open or a member left without a key, a `Welcome`, and an MLS Commit that carries out
a join or a leave, always come after an entry of another member signed on the run
(the leave, the join, the other's epoch); a manager setting a group up writes a few dozen
entries; MLS KeyPackages are spare, never signed on; and only a device that alone writes,
with nobody else writing anything, fills a run over time: a direct chat whose one side
rotates its key every week does so after 78 years, a group of 20 where one device alone
rotates every week (3 KB) after 26, a device alone updating its MLS keys every day (a Commit
of a few hundred bytes) after 11. Then that device writes nothing that counts until another
member does. The comings in and goings out a manager brings stay in its run until a member
in before them writes: in a group of 20 where one manager alone writes for ten years,
rotating every week while each member replaces its device every two years (the old one
leaving or removed, the new one joining with an invite and welcomed), its run holds about
160 entries and 190 KB, 4 % of it.

What no rule on an entry and what is before it can bound: two devices that sign on each
other's entries (a manager with a second device of its own) end each other's runs, as two
honest managers would; only the network budget (3.6) bounds them, and removing either ends
it. The time an author puts on its entries is not used: it chooses it, and making times go
only forward would let one device push everyone's clocks (ticket expiry, time outs, the
weekly rotation).

Some entries can be let go, up to a hard limit: spare entries (`GroupLog::spare`), those
past their author's run and those of a kind that never applies here, that no other entry
needs (none but entries like them is signed on them; an entry signed on them counts them in
its run, above). They take no part in anything, so the
group comes out the same with or without them. A device keeps at most 512 of them (8 MiB)
in all and 64 (1 MiB) of one author; past that, those of the author with the most bytes go,
the oldest to arrive first, which are the least likely to be needed by an entry still to
come. Spare entries signed on one that went then wait for it, within the room for waiting
entries. The node trims each log so every 10 seconds rather than as entries come: an entry
another device needs may be needed here too once the entries after it arrive, in a later
part of the same sync, and let go of at once it would be sent again and again while nothing
after it came. So between trims a log holds at most what the members' budgets bring in those
seconds on top (3.6). A device checks, before letting them go, that the group comes out the
same without them, and keeps them otherwise. Devices compare the heads of the entries that
are not spare to tell whether they need to sync, and send each other only those (and
waiting ones); spare entries reach the members when they are written. One that comes to be
needed (an entry of a later version signed on it, say, or one of a run that an entry of
another member, placed before it, ended) is needed on the device that has it, which then
sends it again, and it stays.

Every other entry is kept: those that apply, and those that apply to nothing in the common
order but fit in their author's run (a change that lost a race, a removal of someone already
out). Its author could make it, so the boxes of an `Add` or `Welcome` among them went to
their devices all the same: a device sent the current secret that way counts as holding it
once it is no member, whether or not it was one then, so a manager starts a new epoch and
proofs under that epoch stop saying anything; it is not counted as given the key (its box may
open to nothing), so a manager still gives it one while it is a member. Each still takes part in the order (joins and leaves count in who came in when), in
the rounds of the losses (removals, leaves and moves down count in how many there are), in
what an earlier round of the losses applied, or in a run, so letting one go could change the
group on one device and not on another. Each is short (a removal or rotation about 140 bytes
per member, 3 KB with 20; the others under 512 bytes, but for a picture an earlier version
set), and the runs above bound them.

An entry of a kind this version does not know (written by a later version) is read whole as
an opaque body: its signature and author are checked like any other's, it is kept, stored and
passed on unchanged, takes its place in the common order (so entries a later version
writes on top of it apply; this one signs nothing on it, as above) and counts in the limits
above, but it never applies and changes nothing. A later
version can therefore add an entry kind without stopping the log on devices that do not know
it yet; they simply ignore what it does.

A removal, a move to the end of the ladder (`Demote`) or a leave outranks every entry of that
device that is neither before nor after it in the graph (and every join with an invite that
device wrote): an entry signed on what its author saw before losing a right could otherwise
be made to sort first and apply with that right. A `Demote` outranks only the entries that
need a place or managing (`Body::needs_rank`: changes to the group, invites, keys given,
moves and removals of others), not the leave or rotations. Room rules apply after the rest
(below), and a loss of their author outranks them the same way. What is outranked
depends on what applies, and the other way round: it is worked out again until nothing
changes (at most once more than the number of losses in the log; past that, the last round
stands, the same on every member). Removals win. So when Ana, alone, sends Bea to the end
while Bea, not knowing, removes Dani and renames the group, the removal and the rename are
undone once the two logs meet, and Dani is a member again.

Changes to which entries a log keeps or applies fork the log between versions: an entry one
version refuses is not kept there, and everything signed on it waits for ever. So the rules
of a level of judgment (`Level` in fogata-state) never change once written, and any later
change to what is kept or applied is a new level, numbered with the capability levels
(3.15): a manager raises the group to it with a `Level` entry once every member's copy
judges at it (the lowest level a member speaks). A group that no `Level` entry raised
judges at level 2, the first with levels in the log. What a log keeps of an entry is
judged on the group as its author saw it, so at the level of the `Level` entries before it
(its ancestors), and every copy keeps the same entries whatever came first; what applies is
judged in the common order, at the level the group has there, as every other rule of
applying, so every copy with the same entries applies the same. Every judgment (what is
kept, what applies, the room rules on top) matches on that level, with an arm for each: a
new level fails to compile until each judgment says what it does there. A copy that does
not judge at the level a `Level` entry names refuses it, and waits on everything signed on
it, as a copy that was never updated, rather than judge those entries by rules of its own;
so a manager writes one only once every member's copy has reached that level, and that
author is not counted as writing what it could not. A level of judgment comes with the
capability level of its number, so the capability level a device lists also says the levels
it judges at; the one place a group's level goes up (`Core::raise_group_level`, which
`GroupLog::raise_level` serves, `append` writing no `Level` entry) checks that every other
member was seen at it. A manager's device raises its group by itself, at a heartbeat, to the
highest level of judgment this version knows that every other member was seen at
(`raise_level_when_all_judge` in fogata-net, the manager above it going first when
connected): a group with a member on an older copy goes as far as that copy judges, and no
further, under the older rules, until that member updates. The group's
settings show its managers that the group waits, and which members have not updated
(`GroupView::level_behind`). One such member could otherwise keep the group under the old
rules for ever (one that stopped using Fogata, cannot update, or connects listing a lower
level on purpose), so once the group has waited a week (`LEVEL_WAIT_MS` in fogata-core,
counted on the manager's device from when it first saw the group wait, kept in `meta.json`
across restarts) its settings offer to raise it anyway, after a confirmation that names who
will have to update before taking part again (`Core::raise_group_level_anyway`, refused if
who is behind changed since it was shown). A week: updates reach a member within a day or two
of its next start, from any friend or member that has them, and a group that meets once a
week sees in that time every member that still uses Fogata. The members left behind refuse
the `Level` entry, keep the group as it was there, and are told that it needs a newer Fogata
(below); nothing of theirs is lost, and they take part again once updated. Copies before levels kept a `Level` entry as a kind they do not know,
changing nothing: that one change came without a level, before any copy was handed out.

Nobody is let into a group that went up whose copy does not judge at its level: such a copy
refuses the log from the `Level` entry on and never gets in, while the group would count it a
member. These are checks of what a manager does, never of what a copy takes. A manager
answers a signed `Join` from a connection listing a lower capability level with `Refused`
(the invite stays good, and the device asks again), checks again when its person says yes,
does not send a group to a friend listing a lower level, and its core adds no friend last
seen at a lower level. A connection that shows no Fogata protocol at all says nothing of its
level, and is answered as before; a joining device shows its protocols to every peer it
shows its invite to; its person is told, once per invite or friend, that that person must
update Fogata first (`NodeEvent::MustUpdateToJoin`). A joining device sent a log with a
signed `Level` entry above every level it knows tells its person to update
(`NodeEvent::UpdateToJoin`). A member's copy given such an entry (a manager raised the group
past it anyway, above) refuses it and waits on what is signed after it, as
above: the group stays there as it was (members, keys, rooms), and what changed since never
reaches it. It keeps the refused entry aside (`GroupLog::level_ahead`): never sent, never
signed on, stored after the log's entries and refused again when the log is read, so the
person is told after a restart too, and an updated version reads it as the raise it is, and
tells nothing. Only a manager's entry counts, as only a manager's `Level` entry is refused
for its level rather than for its author. The group then says, in its room page and its
settings, that it needs a newer Fogata, with the way to get it (updates come from friends and
members, 5f; a button restarts into one once it is ready), and the bell holds a notice of it
until the person dismisses it or Fogata is updated. A manager that wrote an `Invite` before it saw the group go up
to three, which then does not apply, writes it again as a `NamedInvite` with the same ticket
and token (`name_unnamed_invites`), so the link given out works.

Level three (capability level 3) changes two rules, and nothing else: invites name who may
let their device in (`NamedInvite`, 5.3), and a `Rotate` gives the new key to every member
its author saw. A copy of level two refuses the `Level` entry that raises a group to three,
and waits on what is signed after it, so a group goes up only once every member's copy is at
three. The corpus of level two is judged in its test as a copy of level two (one that knows
no later level: the corpus holds `Level` entries naming three, which such a copy refuses),
and still matches `level-two.txt`.

The rotation rule of level three binds only a `Rotate` whose author saw the group at level
three: what is kept is judged on its author's view, at that view's level, and what applies
falls back to level two for a `Rotate`. A member that signs one on entries from before the
raise (when it saw someone gone hold the key) still starts an epoch for itself and a few, as
at level two; and a `Remove`, which a member above another may write, carries a rotation no
level checks for every member. The managers' devices repair either, as at level two: one
left without the key starts a new epoch for everyone (`mend_key`, at most once a minute), one
that has it sends it to the members that lack it (`welcome_lacking`). Level four closes both.

**Level four** (capability level 4) changes two rules, and nothing else:

- a `Rotate` gives the new key to every member of the group as its author saw it (but keys
  nothing can be sealed to);
- a `Remove` gives its new epoch to every member its author saw but the one removed (the
  same keys aside).

One that leaves some out is not kept where its author saw the group at level four; and, where
a raise to four applies, one that the raise did not see does not apply (a `Remove` then
removes nobody, and its epoch does not start), wherever it goes in the order.

*What is judged, and on which view.* Whether the new epoch reaches every member is judged
once per entry, on the group its author saw: the state of the log made of the entry's
ancestors (its parents, theirs and so on), which its hash fixes (`GroupLog::state_before`).
That is the view a `Rotate` of a member that does not manage is already judged on for its
reason to rotate (level two), and the answer is kept with that one, by the entry's hash
(`Seen`), and with what was judged of the log (`Verdicts`), so a log read back at a start
does not work the views out again. Where it holds:

- *kept*: an entry is judged, as every entry, on its author's view at that view's level; at
  four a `Remove` that leaves out a member its author saw is refused (a `Rotate` already is
  from level three), so neither is kept nor passed on, and what is signed on it waits;
- *applied*: the group is worked out holding nothing back; if a raise to four applies there,
  the `Rotate` and `Remove` entries it is not signed on, directly or not
  (`GroupLog::held_to_four`: those its author did not see, written apart from it or after
  it), are held to the rule where they apply, whatever level the group is at there, and the
  group is worked out again holding them, when one of them leaves someone out. Holding only
  those placed after the raise would not do: an entry signed on entries from before the
  raise is placed among them, and goes before the raise when its author came in after the
  authors of the entries it is placed among (5.1's order), so a member could still sign a
  rotation for a few on old entries at any time and have it apply by the rules before.
  Entries the raise is signed on, the raiser saw, as history kept and applied by the rules
  of their time, are not judged again.

Which raise applies is taken from the group worked out holding nothing, so what the rule
refuses (a removal of the raiser, say) never decides whether it holds, and only a raise that
applies holds anything: one by a manager since removed, moved down or gone, signed on
entries from when it managed, is outranked by that loss (as any entry signed apart from it)
and holds nothing; once the group is at four, a second raise is not above its level and
applies nowhere either. A raise on old entries by a manager that still manages (or one that
stopped managing without a loss of its own, the managers set to fewer) does hold what came
after them: that changes nothing of honest history, since every released writer gives the
new key to every member of the group as its copy has it, and its copy has what the entry is
signed on (A raise past two also judges at level three the notes of invites that name nobody
placed after it, as a raise to three always did: they let nobody in by themselves). The only
honest entries that could leave out a member of their view are those signed on more heads
than an entry names (64), which a member must write dozens of entries apart to make, and of
which a member removed or gone in an entry left unnamed is then left out; this version signs
a rotation and a removal on the members of the group its parents make, so it never does.
Tests add raises on old parents, by every manager of then, current and former, at points
through random honest histories (with rotations and removals written as earlier releases
wrote them) and through the saved corpora, and check that honest history applies the same.

*Why every copy agrees.* What applies depends on three things only: the entry's view, fixed
by its ancestors; which raise to four applies in the group worked out holding nothing, which
is the judgment every copy with the same entries already makes alike (what is kept depends
on nothing but an entry and those before it, and what applies on the entries kept); and
which entries that raise is signed on, fixed by its hash. None depends on the order the entries came
in, nor on what else a copy has: two copies given the same entries in any order keep and
apply the same. A property test checks it with random histories raised to three and then to
four, members writing rotations and removals for a few on entries from before each raise and
after them, given to fresh copies as written, the other way round and shuffled, one at a time
or all at once; and checks that every rotation or removal applying after the raise reaches
every member its author saw, that some kept were held back, and that the group worked out
entry by entry is the one worked out again from nothing. The probes of level three's gap
(a member's rotation for itself on entries from before the raise, with and without a
manager's rotation between the leave and the raise) are kept as checks of level four. A raise
signed on every entry goes on top of the group worked out so far (every entry before it is
one it saw); one signed on fewer has the group worked out again, as any entry on old parents
does, and twice only when a rotation or removal it holds leaves someone out.

*What an older copy sees.* A copy of level three refuses the `Level` entry naming four, and
waits on everything signed after it, as for any level it does not judge at: the group stays
there as it was. Entries written apart from the raise and not signed on it it applies by the
rules of three, where copies of four may hold some back, as copies of two and three already
judge apart the entries written apart from a raise to three; it is behind either way, and
told so. Copies up to 0.1.0-beta.9 say nothing of it; this version and later ones tell their
person that the group needs a newer Fogata (`GroupLog::level_ahead`, above). A release that
brings level four therefore sets its `min_build` (5f) to its own build, so that every older
copy is told it must update, from its updates, whatever group it is in. What level three
kept and applied is pinned as level two's is (`level-three.bin` and `level-three.txt`, saved
by the version that brought level three and judged as a copy of level three), and this
version judges that corpus alike until a raise to four in it changes it.

*How a group gets there.* As level three did: a manager's device raises its group by itself
once every other member was seen at capability level four (`raise_level_when_all_judge`), or,
once the group has waited a week, its manager raises it anyway, naming who will have to
update (above).

*The managers' repair.* A member can still lack the current key at level four: one let in
(added or joined) while a rotation that did not see it was written, one whose box opens to
nothing (a box is checked for being there and of the right length, not for what it opens
to), a key nothing can be sealed to. `welcome_lacking` and `mend_key` stay for those, and are
needed less: a rotation that leaves out a member its author saw no longer applies, so it
leaves nobody without a key, and the epoch before it stays current until a rotation that
applies. The device whose rotation did not apply (an honest one only writes such a rotation
when it signs on a view that leaves out members it did not know of, which the rule does not
count against it) is told nothing; while someone gone still holds the key, it rotates again
at its next heartbeat, on what it has then.

The order of the entries, the shortcut that judges an entry signed on every head on the
group as the log has it, and the limits judging reads (members, rooms, names, picture,
entry, parents, runs: `Level::caps`) are behind the level too: the order is worked out at
the highest level a `Level` entry this copy judged names, the shortcut is taken only where
the group's level says the entries it leaves out change nothing, and each level lists its
limits. A level that changes the order changes it for every entry of a group raised to it,
those before its `Level` entry too. How entries are read is not judged by level: a change
to it changes every entry at once, so none is made. The state tests pin all of it: the
order and what level two keeps and applies on the corpus below, and its limits on their
own. Level two's judgment is pinned on a corpus of about 1,300 entries in four worlds
(`fogata-state/src/log/tests/level-two.bin`: random histories of honest and hostile
members writing every kind of entry on whatever parents, each entry also signed by other
devices and an hour later, and a world of the cases below), with what level two keeps,
leaves waiting, orders and applies of it, and the group it makes, in `level-two.txt`: any
change to them fails, and is made as a new level instead. The tests also run fogata-state
as it was at level one (`crates/fogata-state/before`, its sources unchanged, built
only for the tests): on that corpus, where the two judge apart is pinned too, and each such
entry is one of the changes below or signed on one; and random histories of honest devices
with clocks that agree, given to a copy of each as they come, must keep the same entries,
apply the same and make the same group, but for a room rule signed apart from a loss of its
author (each on its own copy), which level one judged by their times and level two by their
places. The changes made between level one and level two (a `Join` dated after its ticket
expired, a member that does not manage rotating for a key sent in an entry that did not
apply, a room rule judged by its place rather than by times, and `Level` entries) are
written by honest devices only so that both keep them, that rule aside, clocks apart aside.

**Later levels: saved logs in place of frozen copies.** A frozen copy of the crate per level
does not scale: level three would need a copy of level two beside level one's, each copy is
code to keep building, and a copy only tests the level next to it. The plan, for level three
on: no new copy of `fogata-state`. Instead, before each release, logs written by that release
are saved as fixtures (the corpus's four worlds, and the logs of the integration and netlab
runs, written by the release's own command line: entries of every kind, on every level it
writes), each with what that release kept, left waiting, ordered and applied of it, and the
group it made, as `level-two.bin` and `level-two.txt` are now. The tests of every later
version read every saved fixture and must judge each the way the release that wrote it did,
at the level the log names, unless the difference is one a new level made and the log was
raised to it. `crates/fogata-core/tests/previous_release.rs` (2.1, 8) checks the other
direction against the last release's command line: what this version writes, the last
release takes in. Once the fixtures of level one's behaviour cover what the comparison with
`crates/fogata-state/before` checks (the random histories of honest devices included, saved
as fixtures), that copy and its pinned hash in `tools/check.sh` can go.

A device keeps the order and the group it worked out until its log changes
(`fogata-state/src/cache.rs`). An entry signed on all the heads goes last and outranks
nothing, so it is applied on top of what was worked out; for any other entry the order is
worked out again from the place after its last parent, and the group from the start when it
is next asked for. A property test checks after every step of random histories that what is
carried forward is what the whole log gives. Boxes holding this device's epoch secrets are
opened once per entry. Where each room rule stands from its author's losses is worked out
with one walk of the graph per rule or per loss of that author (whichever are fewer), and
carried forward over new entries: a rule or loss signed on everything needs no walk, any
other entry placed last one. 800 rules and 800 moves down of their author, which any member
of a group where everyone manages can write, cost about 0.1 s to work out from nothing and
10 ms for each entry after, in a debug build (31 s for each entry when each pair was walked
for each entry, in release).

A log keeps the first `Create` it takes and refuses any other, with or without parents. The
group id is not derived from the `Create`, so without this anyone who knows the id could sign
a second root that sorts first and take the group over. A device that only knows the id (it
is joining with an invite) takes the log a manager sends only if the issuing manager's own
`Invite` entry for its ticket applies in it (5.3).

| Body | Who may write it | Effect |
|------|------------------|--------|
| `Create { name }` | anyone (the root entry) | The author is the first and only member on the ladder (5.2); epoch 0 secret stays with it. |
| `Rename { name }` | manager | New group name (1 to 60 characters, as in `Create`). |
| `Promote { member }` | a member above `member` | Moves `member` up to just below the author. |
| `Demote { member }` | a member above `member` | Moves `member` down to the end. |
| `SetManagers { first }` | manager, never leaving out itself (`first` from 1 to 20, the author among them) | Who manages: everyone (`None`, from the start) or the first `first` on the ladder. |
| `Add { member, name, secret }` | manager | Adds a device at the end of the ladder; `secret` is the current epoch secret sealed to it. |
| `Join { ticket, name }` | the joining device, with a ticket of a manager | Asks to come in with an invite ticket (below). By itself it lets nobody in and uses no ticket: it applies, and uses its ticket, only with the `Welcome` of the manager who lets the device in, signed on it; the device then goes at the end of the ladder. Names of `Join` and `Add` are at most 40 characters. |
| `Welcome { member, secret }` | manager | Delivers the current epoch secret to a member who lacks it; signed on a `Join` of a device that is no member, with a ticket still unused, lets that device in with it first. |
| `Remove { member, rotation }` | a member above `member` | Removes a device and starts a new epoch. |
| `Leave { }` | the leaving member, any, the first too | Removes the author; the ladder closes up and the members then rotate (below). |
| `Rotate { rotation }` | manager; another member only when, in the log as it was when it wrote the entry, a device that is no longer a member held the current secret (given it by an entry that applied, or sent it while no member by one that did not) | Starts a new epoch. Devices rotate for a key sent by an entry that did not apply only when they manage, as versions before those counted need. From level three it is kept only when it gives the new key to every member of the group as its author saw it (but keys nothing can be sealed to), when its author saw the group at level three (5.1): a member could otherwise start an epoch for itself and a few, and leave the managers without the current key. |
| `Invite { ticket, token }` | the manager that issued the ticket; to level two only | Opens an invite: managers announce under its token until it is used or expires (5.3). From level three it applies no more. |
| `NamedInvite { ticket, token, welcomers }` | the manager that issued the ticket; from level three (kind 26, which level two keeps as a kind it does not know) | Opens an invite as `Invite` does, naming the managers (beside the issuer) who may let its device in: only they announce under its token, and only their `Welcome` lets the device in (5.3). |
| `SetIcon { icon }` | manager | One of the preset icons the application draws. |
| `SetPicture { picture }` | manager | A PNG or JPEG of at most 48 KiB shown instead of the icon, or none. Anything else applies but shows the icon. Written by earlier versions; this one writes only a `SetPicture` with none (see the group's picture below). |
| `SetLimits { members, room }` | manager | The most members (2 to 20, 20 at first, never below the members in) and people per voice room (2 to 8, 8 at first). `Add` and `Join` past the first are ignored. A limit set at the same time as an `Add` by a member that came in later applies after it, and is refused if it would leave someone already in out. |
| `SetRoom { id, name }` | manager | Opens a voice room (at most 10) or renames one; names are 1 to 40 characters. No name in the log has control characters or marks that hide in text or turn it around. |
| `RemoveRoom { id }` | manager | Closes a voice room; the first one stays. |
| `Revoke { ticket_id }` | manager | Cancels an invite: nobody joins with it any more. |
| `RoomAccess { room, members }` | manager | Room rule: who may enter a voice room, everyone or only these members (the author and those above it always may). |
| `KeepOut { room, member, until }` | a member above `member` | Room rule: keeps a member out of a voice room until a time by the author's clock, or until let back. |
| `LetBack { room, member }` | as `KeepOut` | Room rule: ends a time out before it runs out. |

`Promote` and `Demote` keep the kind byte (3) of the entry that appointed and demoted admins
in versions before the ladder, with the same flag (1 up, 0 down; any other flag does not
read): those entries read as moves up below their author and to the end. `SetManagers` is
kind 24; read under any other encoding it is kept whole as a kind not known.

Room rules (the last three) are entries like the others, signed and checked against the log,
stored and synced with it, and other entries are signed on them as on any other. Earlier
versions kept them apart (no entry was signed on a rule, and they were stored in
`logs/<group>.rules` and sent with a request of their own, for copies from before them that
could not read them); a log from then still reads, its rules file is folded in when the log
is read and goes once the log is written again. A copy from before this change and a newer
one no longer agree once rules are written: the older one keeps entries signed on a rule
waiting. The rules apply after every other entry, against the group as it then is, the same
on every member: a time out counts only while its author is above the member (one moved up
above its author is let back), and an author that was moved to the end, removed or left
since keeps, for the rules it signed before (that the entry it lost them by is signed on,
directly or not), the place and managing it had then. A rule signed apart from that entry
(neither before nor after it) is outranked by it, as any other entry needing a place: the
author could otherwise sign one on the entries from before it lost its place, dated as it
likes. Earlier versions told this by the entries' times; a rule from the versions that kept
rules apart, which nothing was signed on, whose author lost its place later, applies no
more. A room for some members
also lets in the rule's author and those above it, judged on the same ladder. A time out
ends at a time by the clock of the member who wrote it; each member compares it with its
own clock, so with clocks apart someone gets back a little earlier or later on some
members, and the `Here` it repeats every 20 seconds brings it in everywhere once all agree.

A group's banner (the picture at the top of its column, still or moving) is no entry: a new
kind of entry would make older versions wait for ever on every entry after it, and a log
keeps every entry, so a banner changed often would make it grow. It is a record of its own
(`GroupBanner`, `fogata/v1/group-banner`): the group's id, the author, a time by the
author's clock and a PNG, JPEG, GIF or WebP of at most 550 KiB (none: the banner was taken
away), signed by the author. A member takes it only when the author manages the group in its
log and it is newer than the one it has (at the same time, the larger signature wins), and not
when it is dated more than a day after the member's own clock: one dated far ahead would show
over every banner chosen until then; only the
newest is kept, in `logs/<group>.banner` next to the log, which older versions never open.
Members compare banners through a `banner` digest in their rosters (empty for none), like
the rules, and send theirs with `GroupBanner` to a member whose digest differs; a banner
chosen or received goes at once to the members whose rosters carried a digest. Older versions
send no digest, so they are never sent a banner, and the log they keep is the same. A version
that took banners of at most 512 KiB refuses a bigger one and keeps the banner it has; it is
not sent the one it refused again while it stays connected.

A group's picture (shown instead of its icon) is a record of the same kind
(`GroupPicture`, `fogata/v1/group-picture`, a PNG or JPEG of at most 48 KiB or none), kept
in `logs/<group>.picture`, compared through a `picture` digest in the rosters and sent with
`GroupPicture`, under the same rules as the banner. Earlier versions set it with a
`SetPicture` entry, and a log keeps every entry: every picture a group ever had stayed on
every member, and a member who joined later got them all. Pictures already in a log stay
there (taking them out would change the signed log), but none is added: the picture shown is
the newer of the record and the last `SetPicture` that applied (by their authors' clocks), so
a picture an earlier version sets still shows, and a record replaces it once chosen (it is
always dated after the entry it replaces). An entry dated more than a day after the
member's clock counts as older than any record, and a record or entry dated so is not one a
new record is dated after: a `SetPicture` dated near the end of all time would otherwise show
over every picture chosen after, its author removed or not. A banner or picture read back
from disk is held to the clock the same way. Earlier versions never get the record. So that
they do not keep showing a picture that was replaced or taken away, a manager choosing a
record while the last `SetPicture` left a picture first writes one `SetPicture` with none:
they then show the icon until a manager on their version sets a picture again. That entry
holds no picture and is written only once after each picture set with an entry.

Known gap: banners and pictures have no parents, so two chosen at the same time are told
apart by their authors' clocks, not by the ladder; a member whose clock runs ahead wins
until someone above removes it. Tying a record to the log's heads would let the ladder
decide; it is not done yet.

Names (of the group, its voice rooms and its members, and the names people go by in a room)
are taken as they show. Characters that draw as blank space or as nothing (spaces of every
width, the Hangul fillers, the braille blank, the Mongolian vowel separator, and marks,
joiners, variation selectors and tag characters with nothing to sit on, and format characters
that never draw) become one space between words and none at either end. A name of nothing
else counts as empty: a `Rename` or `SetRoom` under it is ignored, a member or a room
participant under it has no name of its own (the app shows its profile or fingerprint), and
the app does not let anyone choose it. A device signs names already tidied and shows the
others' tidied, so padding does not make one name pass for another. Korean syllables, emoji
sequences with joiners and words that need a joiner or a mark (Hindi, Persian) keep them.

A `rotation` is a fresh random epoch secret sealed separately to every remaining member. If two
rotations happen at the same time, both apply in the common order. A rotation sealed to a
device that is no longer a member at that point is not used for its secret, so a removed
device never gets a newer key; a member that a rotation missed gets a `Welcome`.

Whenever the current epoch's secret is held by a device that is no longer a member (it left,
or its removal came together with another removal whose rotation was discarded) or never
became one, the members start a new epoch. A device never let in holds it when an `Add` or
`Welcome` for it is kept but does not apply: two managers let in two devices with one invite
passed on, one says no while another says yes, the group fills up, or a loss of the manager
outranks it. That entry still went to the device with the log, box and all, so the device
counts as given that epoch's secret (any epoch the group has; a member is never counted so,
since a box for it from someone who could not give it may open to nothing); entries past
their author's run do not count, as they count for nothing else. The member highest on the
ladder among those holding the secret then rotates as soon as it sees the log, and the others rotate after two heartbeats (about 40 seconds) if it has
not, so a member being offline does not keep the old key alive. Two rotations at once are
harmless.

A rotation can carry a box that opens to nothing for some members (it only shows when they
try): those members are left without the current key, and since the log counts them as
holders nobody welcomes them. A manager left without the current key, by a box that does not
open or by none, starts a new epoch for every member at once, at most once a minute per
group, and the device logs who wrote the entry that should have given it the key; while a
manager above it is connected, it waits a minute for that one to do it first, so managers
left without the key together make one epoch, not one each. A member whose box does not open
tells the manager highest on the ladder among those it is connected to (`KeyProblem`, sent
only to devices of capability level 2 or more, 3.15), at most once a minute; that manager
then starts a new epoch, once for each
epoch another device started (its own boxes open), only for a member the log counts as given
that key, and at most once a minute per group.

Keys only come from managers: an `Add`, or a `Welcome` written by a manager (for a first key,
by the manager that accepted the device's `Join` against the clock). Otherwise a member could relay the
join of anyone holding an expired ticket, dated before the expiry, and give it the key, or
mark a member as given a key with a box that opens to nothing, so that nobody gives it one.

A member that does not manage rotates only when someone that is no member holds the current
secret (above), judged by what its author saw, not by the common order, so
that two members rotating at once both apply; any other rotation of a member is ignored.
Versions that did not count a device never let in refuse such a rotation of a member made for
one (it is the first holder, usually a manager, that rotates first), and wait for good on what
is signed on it. And
an epoch whose number another epoch in the log already has is never started (a removal with
one still removes): keys and messages name epochs by their number, so it would take the
other's place.

Letting a device in comes only from managers too. Every member sees each invite's ticket in
its `Invite`, so any member can sign a `Join` with it, under a key of its own, dated before
the expiry and on parents as old as it likes, the `Invite` itself: such a join comes before
the real one in the common order. Were a join to let its device in and use the ticket by
itself, that one would take the ticket, and with it the place, the keys and the epochs of
the device the manager let in (a member could so throw out anyone that came in with an
invite, and use up every invite before it is used); a member's own join signed on entries
from before it was added would put it higher on the ladder; and each such join, counted in
the run of the ticket's issuer, would fill that run until the issuer could write nothing.
So a join waits: the `Welcome` that the manager who let the device in signs on it (on the
join itself, which is no head, so no other entry is signed on it) lets the device in with
it, if its ticket is still unused (the first `Welcome` in the common order wins), and the
join then applies just before it. A `Welcome` lets a device in only with a join it is signed
on, directly or not: the joins nobody let in are spare entries that devices let go, and one
that went on some device could otherwise be used there and not on another. Of those, the one
it is signed on directly (the join the manager saw) goes first, then the first in the common
order. A manager lets in only a join signed on the heads it offered the device, which other
members keep: one signed on entries nobody keeps would make every member keep them, in that
manager's run.

Every version before these rules wrote no entry they ignore (only those who could hand out
keys did, and members rotated only after a leak), so honest logs read the same on older and
newer copies. Copies from before the ladder read the same log with other rules (a creator
and the admins it appointed); there is no public release yet, so they are not kept in step.

### 5.2 The ladder

The members stand in one order, the ladder: the device that made the group is first, and
whoever comes in (`Add`, `Join`) goes to the end. Whoever leaves or is removed is taken out,
and goes to the end again if it comes back. Between any two members one is above the other,
so nobody can act on someone who can act on it: two members never remove each other.

- Everyone acts on the members below it: `Remove`, `Promote` (up to just below itself, never
  above), `Demote` (to the end), `KeepOut`, `LetBack`, and taking them out of a voice room.
- Managing (the name, icon, pictures, banner, limits, rooms and who may enter them, invites
  and letting people in with them, keys given with `Welcome`) is for the managers: everyone
  (from the start, and for groups made before the ladder), or the first n, which a manager
  sets (`SetManagers`) but never so that it or anyone above it is left out. With the first n,
  if they all lose their devices nobody manages the group any more; with everyone, losing any
  device leaves the group as governable as before.
- Anyone leaves at any time, the first too. A leave that reaches nobody (the device was wiped
  first) leaves that member in everyone else's list for good, above those it was above, but
  blocks nothing: the next one does what it did on those below. Nobody can take out the one
  above everyone.
- A `Promote` is only ever signed by the member just above the slot it fills, so two at once
  into the same slot come from one device. A `Promote` and a `Demote` or removal of one member
  at once apply like any two changes made apart: the one of the author that came in first
  applies last, each only if its author is still above the member then. So the member is
  moved down or out, as the other author wanted, unless the one moving it up came in later
  and stands above that author: its `Promote` applies first and puts the member above the
  other author, whose move or removal is then ignored. A `Promote` from someone that came in
  first and a `Demote` at once leave the member just below the one who moved it up (the
  `Demote` applies first). A removal from someone that came in later applies first and
  wins, even against a `Promote` from above it; the one above can bring the member back.

The application shows each member's place ("First", "2nd", "3rd"…), offers the moves, time
outs and removal only on those below this device, and in the group's settings "Who manages
this group: Everyone / The first n".

A device removed by someone whose place was taken at the same time (a `Demote` or removal of
the remover it had not seen) is a member again once the logs meet, but its own device had
stopped running the group when it learnt of the removal. A member that counts it as a member
and is its friend offers it the group over the friend connection as it does any group
(5b); its device sees in the merged log that it is a member by the same entry that let it in
before, and runs the group again without asking its person (a new `Add` or `Join` still
asks: `groups.json` marks the group as an invitation before the log that makes the device a
member is written, so a disk that fails between the two leaves an invitation with the log
from before, which the next offer brings up to date, never a group that runs at the next
start without its person's yes). A device whose removal was undone and that is no friend of any member learns it only
when it is added again.

Group settings have an Activity tab, for every member: who did what, the newest first, 50
lines at a time (`GroupLog::activity`, `Core::group_activity`). It is built from the log, so
every member with the same entries sees the same list: who started the group, added, let in,
removed, moved up or down whom, who left, renames, rooms opened, renamed and closed, limits,
who manages, invites made and cancelled, and the room rules. Keys handed out and rotations
are left out. An entry outranked by a loss of its author (5.1) shows struck through, with
what undid it ("Bea had been moved to the end by Ana at the same time"). The banner and the
picture are records of which only the newest is kept, so their changes come from what this
device saw arrive: each new record is noted in `logs/<group>.history` (who, when by the
author's clock, chosen or taken away; the last 200, always keeping the newest of each),
shown among the rest by time and marked "seen on this device".

### 5.3 Invites

A manager issues a ticket `{ group_id, ticket_id, issuer, expires }`, signed, and notes it in
the log with a random token (`Invite`). The invite link carries the ticket, the group name,
the token and addresses of online members; never the members' own rendezvous secret, so a
leaked invite reveals nothing once it was used or has expired. It also says the level of
judgment of the group's log when it was made (`level`, 5.1): a copy that does not judge at
that level would refuse the log it is let in with and wait for ever, so it tells its person
that the invite needs a newer Fogata as it reads the link, before it tries. The field is one
of its own, in the same format (`v` 3): versions before it skip it and read the rest as they
did, so they still open links to groups at a level they know (and, for a level they do not
know, are refused by the managers and told by the log, as below); a link without it, made
before, reads as it did. While the invite is open, the
managers announce themselves on the DHT under a rendezvous secret derived from the token, and
the invited device looks there and announces itself too. The device dials all the addresses
the invite gives for a member at once and keeps the first that answers; one by one, a first
address that does not answer (another network's, or, on Windows, the connection a start just
before made from the same port, which the system holds for a while after it closed) left the
rest untried. The new device finds a manager, signs a `Join` with the ticket, and the manager
adds it to the log with a `Welcome` signed on it, which lets the device in, uses the ticket
and seals the current epoch secret to it. Members who are
not managers show the device its refusal, so it keeps looking until a manager is online. The
join is kept on the device (`joins.json`) until a manager answers or the invite runs out, so
it goes on if the device starts again meanwhile. A ticket works once and expires (the
application makes them for 24 hours); a manager only lets a device in and gives it a first
key while the ticket is valid by its own clock, and only to a device whose `Join` it let in
itself. The time on the `Join` is not checked against the expiry: the joining device chooses
it, so it would only turn away a device whose clock runs ahead. The joining device dates its
`Join` at the ticket's expiry at the latest, as versions that refused a later one need. A manager can also
`Add` a device whose key it already knows (the application only offers friends).

An invite does not let anyone in by itself, since a link can be passed on: the manager the new
device reaches holds its signed `Join` and asks its person, showing the name and the
fingerprint (`JoinWaiting` to the device). A yes writes the `Welcome` on the `Join`, which
lets the device in, and sends the device the log (`Admitted`); a manager without the current
key yet lets it in once it has one, and the device, which keeps showing its ticket, then gets
the log; a no writes a `Revoke` of the ticket, so the link stops
working for anyone, and tells the device (`JoinDeclined`, with the `Revoke`, see below). The device keeps asking the other
managers meanwhile, and the first answer counts; the others forget the request once the log
settles it. A manager keeps at most three devices waiting with one invite, and sixteen in all;
others are refused, and ask again, until those are answered, so a link passed around cannot
fill a manager's screen with requests. A device let in while it was away is sent the log when
it shows its ticket again, with a `Join` signed by the same key on the heads it was just
offered, so a `Join` copied from the log is not enough.

A signed `Join` proves who wrote it, not who hands it over: the device looks for managers
among whoever is announced under its invite, and any of them could pass its `Join` on to a
real manager, which would then send the group's whole log (names, members, keys, rooms, open
invites) to that peer once its person says yes to the right name. So the joining device signs
again, with its device key, the `Join`'s hash and both transport ids of the connection it
hands it over on, its own first (`fogata/v1/join-commit`, each id after its length;
`JoinCommit` field `binding`), and a manager takes a `Join` only with a binding to the peer
that handed it over and to itself; the log goes only there. The binding travels beside the
entry, not in it, so the log and every version that reads it stay as they were. Versions
before it send none and are refused by managers that check it: an older joining device keeps
showing its invite to them until it runs out, and joins through a manager of its own version
or once it is updated. A release carrying the check sets its `min_build` (5f) to the first
build that sends the binding, so older copies are told they must update. A device shows its ticket to a
connected manager once, so an ask lost on the way (its connection closed while another
stays, or no answer in time) is made again at the next heartbeat, and the manager sends
`Admitted` or `JoinDeclined` again when it is lost the same way (three times at most; then the
device gets it when it next shows its ticket).

A long log goes to the device let in the same way, in parts of at most 256 KiB, each sent
once the one before arrived (tried again three times if lost); the device takes the log
once it has every part, at most 64 parts and 16 MiB. A part sent again (its answer was lost)
takes the place of the one held, and is counted once. Any peer announced under the invite
can say it let the device in, so the parts of all of them together, in every group being
joined, are held up to 32 MiB: past that, those holding the most give way first, as long as
they hold more than the peer whose part came would, so one peer cannot keep a manager with a
shorter log out. Parts go with the connection of the peer that sent them, with a manager
that refused the signed `Join`, and with the join once it is over. A log that once had to arrive whole
within 20 seconds could not be joined below about 1.3 Mbit/s (a member's relay, a phone's
connection). A group offered by a friend (5b) goes in the same parts, at most 4 MiB in all.

The joining device only takes `Admitted` from a manager it sent its `Join` to, and only a log in
which the issuing manager's `Invite` for its ticket applies: someone who saw the invite could
otherwise answer with a group of its own under the same id.

That entry does not tell the real group from a copy of it made by a manager removed or
demoted while the invite was open: that manager keeps the log up to the invite, the
issuer's entry included, knows the token, and in a copy made of that log still manages the
group, so it can let the device into a branch only it answers, with the group's name and
history, where the issuer's `Revoke` never comes. From level three (5.1) the invite says who
may let its device in: a manager writes it as a `NamedInvite` (kind 26, read only by levels
that know it: level two keeps it as a kind it does not know), which names, beside the
issuer, the managers above the issuer on the ladder; an `Invite` that names nobody applies no
more. Only the issuer and those named announce under its token and take its joins, the group
applies only their `Welcome` letting its device in, and the joining device takes a log in
which it is a member only when the entry that made it one is the issuer's or a named
manager's: the `Welcome` on its `Join` with that ticket, or an `Add` after the invite. Every
way in counts, since the copy can as well add the device it learnt from the `Join` it was
sent and seal it the copy's key; a membership from before (whose `Leave` the copy leaves
out) is none the invite allows. The device judges that on the invite's own words, not on the
group's level where the entry applies: a copy can sign its entries on the group as it was
before it went up to level three, and be judged at level two there. A log judged while
another was being taken is judged again (the manager is asked again) rather than added
to it unjudged: together they could let the device in where neither does alone.

The issuer also cancels (`Revoke`) its open invites when a manager is removed or demoted
after them, or the managers are set to the first few, and when a manager that could answer
one leaves by itself: one the invite names, or, for an invite that names nobody (level two),
any manager, where everyone manages too: such a manager could let the invited device into
the copy it keeps. The issuer may be offline then, so the change itself ends them at once:
the manager that removes or demotes a manager, or sets the managers to fewer, cancels in the
same go every open invite the one who stops managing could answer (those it issued or that
name it, any that names nobody: `GroupLog::cancel_invites_lost_since`), and a manager about
to leave cancels those it could answer before its `Leave` (`cancel_invites_before_leaving`).
The device invited believes only its issuer's refusal, so each issuer confirms these with
its own `Revoke` once it sees them (as for another manager's no, below); an issuer removed
confirms nothing, and its invited device, which no manager of the group lets in any more,
waits until the invite runs out. Moving an account to another computer changes no device
key, so nothing ends then. That is only what managers write: every version takes a
`Revoke`.

Why those managers: whoever an invite names can still make such a copy if it loses its
rights while the invite is open, so naming is trust for the invite's life (a day). The
managers above the issuer can already remove it and cancel its invites, so naming them hands
them nothing new; the managers below it, whom the issuer itself can remove, which is the case
above, are left out. The group goes on while the issuer is offline (messages, calls, keys,
every other change); only the device invited waits until the issuer or a manager above it is
online to be let in, and an invite of the first manager on the ladder waits for that manager.
Naming every manager would keep today's reach but protect from nothing: the one removed is
one of them. A group below level three (one with a member whose copy does not judge at it)
keeps the invites that name nobody, and that limit.

A no is checked the same way. The device knows only the manager who wrote its invite (the
ticket names it), so it believes a refusal only when it carries that manager's signed
`Revoke` of its ticket (`JoinDeclined` with the entry); anyone else who learnt the group's
id, a member included, could otherwise end the join. A refusal it cannot check is logged and
ignored, and the device goes on showing its invite. When another manager says no, the issuing
manager's device, once it has that `Revoke`, writes its own for the same ticket (an invite
nobody joined with); every manager then answers the invite with it, and managers still holding
the join send it on their own. The refusal is bound to the ticket, not to the device: a
ticket is single-use and the `Revoke` ends it for whoever holds it, so a member passing on
the issuer's `Revoke` only tells the truth. Devices that check refusals end a join a day
after the invite expired by their own clock (managers decide by theirs, which may be behind),
so it never waits for ever.

Older devices: the refusal also goes as before, an unsigned `JoinDeclined` answer to the
invite, and the `Revoke` travels in a field older devices skip, so an older device being
refused still learns it at once. A device that checks refusals, refused by an older manager
(which sends no `Revoke`), or by an updated manager while the issuing manager is older (which
never confirms another's no), keeps waiting and asking until the issuing manager's `Revoke`
reaches it or the invite runs out; nothing breaks either way.

### 5.4 Epochs and keys

Each epoch has its own random secret; `auth_key`, `message_key` and rendezvous keys derive from
it and the group id. An epoch lasts a week at most: once the current one is seven days old
(from the time the device that started it put on its entry, or from when this device first
saw it, if that was earlier), a manager online starts a new one (each manager
after the first, in the order of the ladder, an hour later than the one before, so they do
not all rotate at once). Whoever starts an epoch chooses that time: one dated more than a day
ahead of a manager's clock counts as a week old once the manager sees it, or a time near the
end of all time would keep the group on that key for good. In a direct chat's group the
friend who made it first starts the new epoch, and a call's group never rotates. That bounds how long a stolen epoch key reads new messages; it is
not forward secrecy, since the older secrets stay in the log sealed to each member's device
key. A rotation adds about 140 bytes per member to the log (1.3 KB with 8 members, 3 KB with
20, so about 70 KB and 160 KB a year). Messages carry their epoch. Members keep the secrets of earlier epochs to
read messages in flight. Membership proofs state their epoch; a peer is accepted only if its
device is a member in the current state, whatever epoch it proves with, so a member who was
offline during a rotation can still connect and receive the new secret through the log.

Members announce and look up the rendezvous of the current and the previous epoch, so members
that missed a rotation still find the group. A removed device can therefore see members'
addresses for one more epoch, but cannot join or read anything new.

A removed member keeps whatever it already saw. Stronger guarantees (recovery after a device
compromise, efficient rotation for large groups) are what MLS adds; this design keeps the
same membership model so it can move to MLS later.

### 5.5 MLS (in progress)

Groups are to move their keys to MLS (RFC 9420) in "exporter mode": MLS keeps a ratchet tree
of the members' devices, and each epoch's secret comes from MLS's exporter
(`fogata/v3/epoch`, with the group id as context) instead of a random secret sealed to every
member; every key above it (5.4: messages, proofs, rendezvous, rooms) is derived as now, with
the first 16 bytes of MLS's epoch authenticator as the epoch id. Past epochs' secrets are
kept for the last 3 epochs and at most a week after the next one started, then deleted. A
copy of a device's data will then read nothing older than that, and only until the device
next updates its own keys (each device will do so once a week). None of this is used yet: no entry below is written, and
groups and copies work exactly as before.

**What exists (`crates/fogata-mls`, `crates/fogata-state/src/chain.rs`).**

- A crypto provider for OpenMLS on the `hpke` crate and the RustCrypto crates (the providers
  published with OpenMLS depend on `hpke-rs`, MPL-2.0). One ciphersuite: 0x0003, X25519,
  ChaCha20-Poly1305, SHA-256, Ed25519. Checked against RFC 9180's test vectors (A.2.1) and
  against messages recorded from OpenMLS's own provider (`src/testdata`).
- A device's credential is its device key followed by the device key's signature over its
  MLS signing key, which is derived from the device key's secret (never the same key; always
  the same for one device, so a device that lost its MLS state comes back under the same
  credential and MLS replaces its old leaf). Every leaf must carry one that checks.
- Log entries for MLS groups, kinds 20 to 23: `MlsKeyPackage`, `MlsJoin` (a join with the
  device's KeyPackage), `Commit` (the epoch it builds on and the `Commit` entry that started
  it, the devices it adds and removes, and the digests of the MLS Commit and Welcome, which
  are kept beside the log) and `Resync` (an external Commit by a member that lost its MLS
  state). Versions from before them (with unknown kinds) keep them as entries of a
  kind they do not know; this version reads only the encoding it writes and keeps anything
  else under these kinds the same way, so every copy keeps the same entries. Until group
  rules take them, they change nothing.
- The chain of epochs (`GroupLog::mls_chain`): Commits name the Commit they build on, so they
  form a tree. A Commit counts if it fits in its author's run (5.1), if its author may make every change in it, judged on the group
  as its entry saw it (adds by managers, of members not in the tree; removals of devices that
  are no longer members, by a manager or a member that stood above the removed one, or by any
  member for a device that left; updates of the author's own keys at most once a day; resyncs only by a
  device in the tree; the first Commit by any manager), if it builds on a Commit that counts,
  if no removal, move to the end or leave of its author outranks it, and if its MLS message is not known to be bad. Among the Commits
  that count on one epoch, the one with the longest line after it wins, ties to the smallest
  entry hash. Every device with the same entries gets the same chain, whatever order they came
  in (tested over random histories of six devices that meet only now and then).
- A device's MLS state for a group (`GroupMember`): KeyPackages, Commits that wait until their
  entry wins, checking others' Commits against their entries (sender, epoch, and exactly the
  changes the entry names), joining from a Welcome, resync from a GroupInfo, and following the
  chain: on to new links, back to a snapshot (one per link, the last 8, at most 2 days) when a
  branch it was on lost. A Commit or Resync under this device's key that this state did not
  make means another copy of the device took its place. The state is written whole as one
  file per group (`mls/<group>.bin`), sealed with the folder's data key like the other secrets.

**Still to come.** Group rules that switch a group to MLS, KeyPackages in joins and
in the friend book, the Welcome in `Admitted`, the messages beside the log and their
transfer, who commits when, the weekly update of each device's keys, resync over the member
connection, the notice when another copy of a device takes its place, full epoch ids on the
wire, migration of existing groups (without waiting for a member that has not
updated: it adds itself later with a KeyPackage), direct chats, and a way to start the
tree again when no device holds its last epoch any more (every device that had it lost its
state, or the others were on a branch that lost for longer than their snapshots go back).
When it is wired in, a `Welcome` taken to join, and a `GroupInfo` taken to resync, must be
checked against the log: the ratchet tree they carry holds exactly the members the log has at
that link (each leaf's credential one of their device keys), so a member cannot hand a device
an MLS group with someone the log never let in, or without someone it did. Until then nothing
of MLS runs in the application (only the `fuzzing` feature uses it).

## 5b. Profiles and friends (fogata-friends)

`fogata-friends` builds and checks everything below without doing I/O; `fogata-net` runs it
over a second request-response protocol, `/fogata/friend/1`.

**Profile.** Name (1 to 40 characters that show, as in 5.1, and no control characters or
hiding marks; a profile whose name is not already tidied is refused), avatar (one of five
preset pictures shipped with the application, or a PNG or JPEG picture of at most 64 KiB;
the app makes a photo a 256-pixel JPEG, sharp on high-density screens, and keeps a picture
with see-through parts a PNG) and a version (when it last changed), signed by the device
key. A preset number this version does not know (from a newer one) is accepted and shown as
one of its own. Friends receive it when they connect, and a newer version replaces an older
one. Group members send each other their profiles after the membership proof. Both ends
check each other's proof at once, so one may get the other's profile, or whether it appears
offline, before it has checked who sent it: it keeps them until then. A changed profile goes
at once to every friend and member connected, and the newest copy wins. To a friend it goes
one at a time: changes made while one is on its way go as the newest once it has arrived
(a burst of them would take more streams than a connection opens at once, and the ones past
that are lost, maybe the newest), and one that did not arrive goes again at the next
heartbeat. The profiles of
members who are not friends are kept too (`members.json`), so a member's name and picture
show as last met while it is away, also after a restart. A friend or member can sign new
versions as often as it likes, and each one rewrites a file (the friend book holds every
friend's pictures): a device takes at most two profiles, banners and bios from each friend,
and two profiles from each member, in 10 seconds (the one sent on connecting and a change
right after it); one more waits for the next turn, replaced by any newer one meanwhile. The friend book and `members.json` are written at most
every 3 seconds: the first change at once, the rest of a burst together, and whatever waits
when the application quits.

Files the core rewrites as the network changes things (group logs, `meta.json`, the friend
book, `members.json`, animations, capability levels) are written by one thread of their own,
in the order they were asked for, each as before: a temporary file, synced, renamed over the
old one (`fogata-core/src/writer.rs`). A slow disk delays the files, not the events. A write
of a file that still waits for its turn is replaced by a newer one of the same file, unless
something asked for in between must come after it. A change the person makes to a group is
written in its turn and on disk before it is told done. Direct chats' records (journal,
pins, tails, rewrites, the chat's group and the friend's profile) go through the same thread,
asked for in the order the chat changed, so reading a chat never waits for the disk: a
message is sent, and one that came is confirmed, only once its records are on disk, and
items are sent to the friend only once every record asked for before them is written; one
of this device's that could not be written is taken out of the chat before anything after
it is written. A chat written again whole is copied as it is when its turn comes; the
records asked for after that copy are added to it again, which reads the same (each says
what holds, by id). When the core shuts down, it writes
the log of every group it runs as the node has it, lets the node's last events be handled,
and waits for every write.

**Fingerprint.** The first 80 bits of SHA-256 of the device key in Crockford base32, shown as
`7Q4M K2XR 9PLD 3VHC`; the short form is the first eight characters. People compare it when
they become friends: each device prints its own, and the other device shows it next to the
request.

**Friend secret.** Every device has a random 32-byte friend secret that only its friends
know. It announces itself on the DHT under rendezvous points derived from it (6-hour windows,
as for groups), so only its friends can find it.

**Becoming friends.** Device A makes an invite; device B uses it; B's request reaches A; A's
person compares fingerprints and answers; on acceptance both keep each other. An invite is
valid for 10 minutes and accepts one request.

- *Link or QR code* (`fogata:friend:...`, about 240 characters): A's device key, name and
  transport identity, up to four addresses (those others confirmed first) and a random
  128-bit token. A announces under a rendezvous point derived from the token. B's request
  carries an HMAC keyed by the token over both transport ids, which proves B saw the link,
  and, sealed to A's key, B's signed profile, a one-time request secret and B's signature
  over both transport ids.
- *Short code* `FGT-XXXXX-XXXXX` (ten Crockford base32 characters: 49 random bits and the
  parity of the UTC hour it was made in): too short to carry a key or an address. Both sides
  stretch it with Argon2id (64 MiB, 3 passes; about 0.13 s on a desktop), salted with that
  hour, into a secret from which come the rendezvous secret, a gate key and a SPAKE2
  password. The device that uses a code takes, among the hours with the code's parity, the
  one nearest to five minutes (half a code's lifetime) before its own clock, so it stretches
  the code once and is right while the two clocks differ by less than about 25 minutes. B
  opens a run with a gate (an HMAC over both transport ids), so that strangers cannot use up
  A's runs. A answers with its SPAKE2 message and its profile, encrypted with the run's key;
  B sends its profile and a request secret the same way. Someone watching a run learns
  nothing, and A stops answering a code after three runs that ended badly (a last message
  that does not open, or none within two minutes); a run B opens again while its earlier one
  waits (an answer lost, a connection dropped) does not count. What really
  protects a code is the cost of guessing it: DHT nodes that see its rendezvous points can
  test guesses offline at one Argon2id run each, so finding a code within its 10 minutes
  would take on the order of 10^11 CPU cores, and because of the hour in the salt a table of
  every code computed in advance (some 2 million CPU-years) serves one hour only.
  Codes of an earlier format, `FGT-XXXX-XXXX` (40 bits, one fixed salt for every code), are
  refused as not a code; only builds from before any release made them.
- *From a group* (no invite): a member can ask another member of a group they share from
  its card. The request (`FriendAsk`, a member request) goes on a connection where the
  other proved to be that member, and carries what a link's does, without the HMAC: B's
  signed profile, a request secret and B's signature over both transport ids, sealed to A's
  key. A takes it only from a peer that proved membership of a group other than a direct
  chat's, and only when the profile inside is that member's device; from there it is an
  ordinary request, answered and completed as one made with a link. A shows a member's
  request at most once an hour (one declined a moment ago is acknowledged and let go,
  telling nothing, as a decline never does); a member that asked first is accepted when
  the other asks back. Older devices cannot read `FriendAsk`, so it goes only to those that
  advertise `/fogata/member-friend/1` in identify; for an older or unconnected member the
  person is told to try later or send a link.
- *Both ends look for each other.* Getting through two NATs needs both ends to dial at the
  scheduled slots (3.4), so the device showing a link or code looks up the peers that
  announce under it, and the device that uses it announces there too. A link's addresses
  are tried first; on the same network, mDNS peers are offered the code run.
- *Answering.* A request waits for the person's answer, also across restarts (`fogata
  friends accept`). Declining tells the other device nothing. The acceptance carries A's
  profile and friend secret, sealed to B, with an HMAC keyed by the pair secret of A and B
  (as every acceptance and every new friend secret does); B then
  sends its own friend secret the same way. A request never carries the friend secret
  itself, so a device that never accepts cannot follow the requester afterwards. While a
  request waits, B announces and looks up under the pair secret it shares with A (below);
  if B is gone when the answer comes, A announces there too, and they find each other. A
  request waits 30 days
  at most (A drops it a day earlier, in case B's clock is ahead): A holds the request
  secret, and could otherwise look up where B is for as long as it never answers.

**Finding friends.** A device looks up the rendezvous points of its friends that are not
online: every 30 seconds during its first 2.5 minutes and while something waits for an
answer, otherwise every 2 minutes. Friends that start at about the same time need the early
lookups, because neither has announced itself when the other first looks. Every friend of a
device knows its friend secret and could announce there too, to be dialled and learn the
address and transport identity of whoever looks (an ex-friend of this device that is still
a friend of that one, say). So each announcement carries one more sealed name (3.3): the
device's signature over its transport identity and the rendezvous window, sealed with
`seal_short` (here up to 64 bytes) under a key from its friend rendezvous secret, 160 hex
characters in three DNS labels. A device dials a peer found under a friend's own secret only
if that name opens and the signature verifies as that friend, for that peer and a window
from two before to one after its own; anyone else is skipped without being dialled. On a
new connection the device that found the other sends a hello: an HMAC keyed by the pair
secret the two share (below) over both transport ids and, sealed to the friend, its device
key, a signature over both transport ids, and an HMAC keyed by the friend's secret as this
device knows it, which tells the friend which of its secrets this device has. The friend
tries the pair secret of each of its friends, and answers only if that device is a friend,
with its own signature, sealed back, and its profile if the other's copy is older. A stranger
who reaches a device learns nothing about who it is; nor does another friend of the friend
that answered a probe with the friend's tag (it knows the friend's own secret, not the pair
secret), although that a hello came at all tells it the device recognised the tag. On the local network a device asks each peer for a
probe tag, an HMAC keyed by that peer's own friend secret over a nonce and both transport
ids; only the peer's friends recognise it. Online friends exchange a heartbeat every 20
seconds, which keeps the connection open and shows presence.

**Banner.** The colour (0xAARRGGBB) and the PNG or JPEG of at most 64 KiB at the top of a
device's card, with a version, signed by the device key apart from the profile: it goes to
friends only, never to group members. Each connection to a friend carries it once, and a
changed banner goes at once to the friends connected. The application sends a narrower copy
of the picture the person chose, small enough to fit.

**Bio.** A few lines under the name on a device's card: at most 190 characters (line breaks
included) on 4 lines, each line tidied as names are (5.1), blank lines at either end left out
and those in between kept as one; line breaks are the only control characters allowed, and
a bio with any other, or with a hiding mark, is refused. Like the banner it is signed by the
device key apart from the profile (`fogata/v1/bio`), with its own version, and goes to
friends only, sealed to each (`FriendRequest::Bio`), once per connection and at once when it
changes; a received bio that is not already tidied, or not from the friend on that
connection, is refused. Only friends see a person's card, which is why the bio goes where
the banner goes and not to group members. It is not a field of the profile because every
earlier version refuses a profile with anything after its signature, also inside the group
handshakes and the friend book, so a profile with a bio would stop reaching them. Earlier
versions cannot read `FriendRequest::Bio` either, which fails only that request (the
connection, presence and everything else carry on), and the sender does not send it again
on that connection. A device that never wrote a bio has none: cards then show the
application's default line, "Hey there! I'm on Fogata."; an empty bio shows none. The book
keeps the device's own bio and each friend's (book version 5 and later; versions 1 to 4 still open).

**Animations.** A profile picture or a card's banner can move: a GIF or WebP of more than
one frame, at most 550 KiB (earlier versions took 256 KiB for a picture and 512 KiB for a
banner, and refuse a larger one: they show the still picture), at most 500 frames and at
most as many pixels as 512 by 512 for a picture, 1024 by 384 for a banner (a group's
banner, 1024 by 576, in the application). Every frame is decoded at the animation's whole
size however small it is drawn, so the core reads the size and the frames from the GIF's
blocks or the WebP's chunks, decoding nothing, and refuses a larger record as it refuses a
heavier one; earlier versions took up to 2048 by 2048 with any number of frames, and one
of theirs past these shows as the still picture here. Every earlier version
refuses a profile or banner whose picture is not a PNG or JPEG (inside the friend handshakes
too), so the profile and the banner keep a still picture, the animation's first frame, and
the animation goes apart: a record signed by the device key (`fogata/v1/animation`) naming
what it moves in place of (picture or banner) and the SHA-256 of that still picture, with
its own version; an empty one says the device shows none. It goes to friends sealed to each
(`FriendRequest::Animation`, picture and banner), once per connection and at once when it
changes, and the picture's to members inside `MemberRequest::Profile` (field `animation`).
Earlier versions fail `FriendRequest::Animation` and do not read the field: they show the
still picture. The core keeps the newest record of each device (at most 512, in
`animations.bin`) and gives the application one only while the device's picture or banner
is the still picture it names; one more of them is written only while the disk has room for the
new file and 256 MiB to spare (it is a copy of what the network gives again: refused, the
file stays as it was), fewer always. It keeps them, and the members' profiles (`members.json`),
only of friends and members of its groups: those of a device that is neither any more go at
the start and a moment after a friend, a member or a group goes, and so do the banner and
picture of a group the device left or was removed from (its log stays, to tell the group if
it is added again). The application keeps the animations of the 48 people shown last (of
pictures, and of banners), and forgets those of a person no longer a friend nor a member. The application plays it only in avatars of 48 points or
more (cards, the person's panel, call tiles, a direct chat's header) and in banners, decodes
frames at the size drawn, and steps them only while the picture is on screen and the window
is seen and focused, and only 6 at once: the others stay on the frame they reached (the
first, for one that never played) until one of those playing stops; the small pictures of
lists stay still, and so does every still picture, a PNG that moves included (only its
first frame is drawn). The device's own animations
stay in its data folder (`photo.anim`, `banner.anim`) and are given to the core at each
start.

**Removing a friend** forgets it and makes a new friend secret, so the former friend can no
longer find the device or recognise its probe tags. The remaining friends receive the new
secret like an acceptance: at once if they are online, otherwise the next time both are.

For that the two meet under a **pair secret**, which only they know: X25519 between their two
device keys (the same on both sides; a key of small order agrees nothing), through HKDF-SHA256
with the label `fogata/v1/friend-pair` and both keys in order. A device announces under the
pair secret of each friend that does not have its current secret, and of each device a
request made here went to; and it looks up, besides each friend away under that friend's
own secret, the pair secret of each friend away, former friend being told and device asked.
Earlier, deliveries met under the friend's own secret, which the friend's other friends
know: a former friend still friends with one of them found the device's announcements
there (its public address, its sealed names) and was dialled as that friend. Under a pair
secret only the two meet, so no proof is asked of what is found there (a DHT node on the
lookup path could still answer with a peer of its choice, as it can for anything; it saw
the device's address in that lookup anyway). Each friend away costs two lookups instead of
one; with every friend online nothing is looked up at all. The acknowledgment of an acceptance carries no proof, and anyone who knows
that friend's secret (another friend of it) can announce there or answer probes with its
tag: so it counts only from a peer that proved it is that friend (its answer to the hello
that follows, or a hello whose sealed HMAC is keyed by the new secret); until then the device
goes on announcing.
The former friend is also told, so it forgets the device too: a removal
signed by the device and sealed to the former friend, over the connection it has now, or
else kept in the book's `departing` list and sent once the device finds it again (it looks
it up under the former friend's own secret, which it still knows, for up to 90 days). The
former friend answers with its signature over both transport ids and the removal's SHA-256,
sealed to the device (`FriendResponse::Unfriended`); only that answer ends the removal, so
another friend of the former friend, which knows its secret and can answer where it is
looked for, cannot stop it from being told. It answers so only a device it knows: a friend,
a request either way, a former friend it is telling, or one that removed it in the last day
(its first answer lost): the book keeps those, at most 64, with when each removed it, so this
holds across starts (book version 6; versions 1 to 5 still open, and an earlier version
refuses a book of version 6 as one saved by a newer Fogata, as with every version before). Anyone else gets the plain refusal every device gives a
removal that is not sealed to it, so someone who knows a device key (a former friend long
gone) cannot send removals sealed to it to peers it meets and learn, from a signed answer,
which of them is that device. The former friend forgets the device and makes
a new friend secret as well. Nobody gets a message: the friend simply disappears from both lists. Becoming friends
again before the removal reached the former friend drops it from `departing`: told later,
it would undo the new friendship on one side only.

Until every remaining friend has the new secret, the device keeps its earlier ones (the last
four, for at most 90 days) and accepts hellos and acceptances keyed by them. Two friends who
both changed secrets while apart each know only the other's earlier one, so neither
recognises the other's announcements or probe tags. Over the DHT they meet under their pair
secret, where both announce while each waits for the other's new secret. On the local
network the device greets a peer whose probe tag it does not know with a hello to each
offline friend still waiting for its new secret (four at most). A hello is keyed by the pair
secret and sealed to that friend's key, so only that friend can open it, and nobody else
learns from it whom the device is friends with. Probe answers carry a tag under the current
secret only: a former friend knows the earlier ones. A friend whose secret this device no
longer knows at all (after more than four changes, or more than 90 days apart) is still
recognised by its hello, which needs only the pair secret; on the DHT it is found under the
pair secret when it has a new secret to deliver.

**Friends in common** are found without either device showing its list. The asker hashes
each of its friends' keys onto the Ristretto group and raises them to a secret of its own;
the friend asked raises those to its own secret and sends them back with its own friends
hidden the same way, in a random order; the asker raises the latter to its secret, and the
keys that match are the friends in common. Both lists are padded with random points to a
multiple of 16, so each side learns only roughly how many friends the other has. Only a
friend connected now can ask, and gets an answer at most once an hour and for at most
4,096 keys asked about in a day (a device with up to 112 friends still gets one every hour,
one with a thousand four a day; the application keeps showing the last one). The asker
chooses what it sends: a modified application could put keys it already knows in its list
(up to about a thousand a question, 4,096 a day) to learn whether each is a friend of the
other; it cannot learn anything about keys it does not know. Answering only for the asker's
real friends would need each friendship to carry a proof the asker could show without
showing whose it is, which is a protocol of its own.

**Groups with friends.** A group manager adds a friend directly (an `Add` entry with the
friend's device key and the current epoch secret sealed to it). The next time both are
online while the manager's device has the group open, it sends the group log over the friend
connection; the friend checks that it is a member and keeps the group. A long log goes in
parts of at most 256 KiB (`FriendRequest::Group` with `part`), each sent once the one before
arrived (tried again three times if lost); the friend reads the log once it has every part,
one log per friend at a time, at most 64 parts, 4096 entries and 4 MiB. The parts of all
friends' logs together are held up to 16 MiB; an offer whose next part has not come in two
minutes, or that is not whole ten minutes after its first part, is dropped, and when the
offers would hold more than that, those holding more than the friend whose part came would
give way, the most first, so a few friends that never finish keep nobody else's offer out.

## 5c. The application core (fogata-core)

`Core::start` opens a device's data folder and starts one node with every group the device
is a member of and its friends. A background task turns the node's events into saved files
and screen events: a group log is saved when it changes, the friends file when friends
change, a group a friend shared or an invite brought is registered under a free local name,
and two kinds of times are kept for the screens (when a friend was last seen, when a group
last had a room). Nothing said or shared in a room is written; direct chats are kept in
`direct/` (3.13). A group this device left is gone from the screens at once, but the node
runs it on until a member has the leave: one that takes it when it is pushed, names it among
the entries it has when it syncs, has the same log heads, or writes an entry on it. A member
that has not counted this device in yet refuses the push, and one away gets nothing, so the
leave goes with the next sync instead of waiting for the next start. Only when nobody else is
left in the group does it stop at once. The node does not say when a member got it, so the
group still runs at each start for two weeks after the leave (members away meanwhile learn it
when they meet this device), without showing; an invite to it still lets this device join
again.

What the screens read (all computed on demand from the node and these files):

- `groups()`: each group with members, members online, this device's place on the ladder,
  whether it manages, who manages, those below it, its
  live room (participants, start) or when it last had one. Live rooms first.
- `friends()` and `requests()`: each friend with fingerprint, avatar and presence (online,
  in a room with the group named only when shared, or last seen); requests both ways.
- `room(group)`: participants (and who seems away), the chat so far, and the files with who
  has them, download progress and how many times this device sent each.
- `members(group)` (each with its full fingerprint, shown on hovering the short one),
  `network_report()` with `diagnose::diagnose` for plain words.
- `direct_chats()` (each chat with its last line, unread count and timer) and
  `direct_chat(friend)` (its items with each attachment's state, the messages pinned,
  whether the chat is connected, what it takes here), `direct_storage()`.

What they do: make a profile, change it; friend links and codes, add, answer, remove; make a
group with friends, invite, join, add or remove members, move them up or down, choose who
manages, rename, leave; enter and
leave a room (one at a time), talk, share a file, stop sharing it, download one; write to a
friend, change a chat's timer, pin or unpin a message, fetch or save an attachment, clear a chat's files, delete a
chat that can only be read, change the settings of direct messages. Events say when to look
again (`GroupsChanged`, `FriendsChanged`, room changes, members connecting, requests,
downloads, `DirectChanged`, `DirectMessage`).

The CLI's `chat`, `friends invite|add|online` and `group join` run on the core, so they keep
every group and friend online, as the application does.

**Data at rest.** A data folder's secrets are sealed with a random 32-byte data key that the
system keychain keeps (`vault/mod.rs`, through the `keyring` crate): Keychain Services on macOS
(the login keychain), the Credential Manager on Windows (protected with the user's DPAPI
key), the Secret Service elsewhere (GNOME Keyring, KWallet), under the service `Fogata` and a
random name per folder. The Flatpak is not let reach the Secret Service, whose name would give
it every secret the person keeps there: it asks the Secret portal (`org.freedesktop.portal.Secret`)
for the secret the desktop's keychain keeps for this application alone, and keeps each data
key in a file of its own in `fogata-keychain/` of its data folder
(`~/.var/app/dev.holypanda.fogata/data`), sealed (XChaCha20-Poly1305) with a key derived from
that secret by HKDF-SHA256 and bound to the key's name (`vault/file.rs`,
`vault/secret_portal.rs`). A key an earlier Flatpak kept in the Secret Service is moved there
the first time a folder asks for it: written whole in a new file and renamed, read back the
same, and only then used; the Secret Service's copy is left as it was, so a move cut short
anywhere is made again at the next start and nothing is lost. A Flatpak that does not reach
the Secret Service any more (a version without the permission) and finds a folder whose key
is only there opens nothing and says how to let it reach it for one start (`flatpak override
--user --talk-name=org.freedesktop.secrets dev.holypanda.fogata`, then `--no-talk-name` to
take it back). Sealed are the identity (`identity.key`), the friends file and its
copy (`friends.bin`, `friends.bin.bak`), and per direct chat the journal, the pins, how
the two end (`tails`), the friend's profile and the attachments (3.13); each with XChaCha20-Poly1305 and what the file
is as associated data (`fogata_crypto::at_rest`): one box for a whole file, a box per record
for a journal, bound to the record before it, 64 KiB chunks for a stream, whose last chunk
says it is the last. A chunk's nonce is the stream's random prefix (96 bits), the
generation of the writer that sealed it and its number: a download cut short is taken up
by the next generation, which writes its number in the stream's header and makes it reach
the disk before it seals anything, and leaves the chunks already there as they are, so a
nonce never seals twice and a retry does not write again what came. One prefix serves 32
generations; the next writer seals what is kept again under a new prefix (as it does with a
stream written before generations) into a new file beside it, which takes the old one's
place in one step once it is on the disk, so a computer stopping in the middle loses nothing
that came; a reader tries at most those 32 generations for a chunk. Group logs
are not sealed, as what is secret in them is sealed to the identity; nor are the list of
groups, the member profiles, `meta.json`, `joins.json`, `app.json` and `videos/` (room
videos fetched to play, gone when the room is left), which hold names, addresses and
settings; nor `diagnostics/` (the log of the last days, 5d), which holds nothing personal
by the time it is written. `sealed.json` names the folder's key and holds a box only that key opens: a
sealed folder never opens without it (a keychain that cannot be reached, lacks the key or
holds another one is an error that says so and changes nothing). The identity and the
friends file name the key too, in the clear before their box: a folder that lost
`sealed.json` asks the keychain for that name (keychains give what is asked for by name but
cannot list what they hold), and a key that opens them gets a new `sealed.json`; files sealed
before they named their key are sealed again with its name at the next start. The application seals; the
CLI only opens sealed folders, so test folders put nothing in the keychain, and
`FOGATA_SEAL=off` makes a debug build of the application do the same (for test copies); a
release build ignores it, so nothing around it can leave a new folder plain. The other
switches for test copies (`FOGATA_DOWNLOADS`, `FOGATA_TEST_OPENED` which writes the links and
folders that would open into a file, a made-up camera, field trials, the program whose sound
a shared screen takes, media over streams) are ignored by release builds too; the application
reads its own through `testSwitch` (app/lib/src/test_switches.dart), which a test keeps so.

A folder is sealed the first time the application opens it while no other copy runs on it,
when the keychain keeps and gives back a new key. With no keychain (no Secret Service on a
bare window manager, say; or one locked, or giving back another value) a new folder gets
nothing secret written: the welcome page asks first whether to try again (once the keychain
is started or unlocked) or to go on unencrypted, knowingly (`unencrypted.json` notes the
choice; an account moved in is written plain only after it too). While another copy runs
on a new folder (a second Fogata, or the CLI, which never seals and so writes a new
folder's identity plain), the application writes nothing in it and says Fogata is already
running there. A folder left plain is
tried again at each start, and sealed at the first one a keychain keeps a key; meanwhile
the settings (at the top of Storage & downloads), and the bell at start until the notice is
dismissed, say that this computer's data is not encrypted and, on
Linux, that a running Secret Service gives it a keychain (`at_rest`: not with
`FOGATA_SEAL=off`, which leaves it plain on purpose). Sealing
copies each plain file to `before-sealing/` first, seals it in its place and reads it back
(a file that does not read back as it was goes back to plain); the identity and the friends
file go first, so a copy of Fogata from before sealing refuses the folder from then on (it
reads the friends file as written by a later version, 32 being the first byte of every
sealed file) instead of changing it. A start cut short goes on at the next one, each file
being sealed or plain on its own. `sealing.json` (which stays on this computer when the
account moves) says how far it got: `pending` before the first file is sealed, `failed`
with why (a copy that cannot be made in `before-sealing/`, a full disk, a file that does not
read back), and `done` only once every secret in the folder (the identity, the friends file
and its copy, the MLS state, each chat's journal, pins and profile) was found sealed and
opening with the key; copies of damaged files need only be sealed. Until then the folder
does not count as sealed (`Store::sealed`, `Sealing::Unfinished`), and when `sealed.json`
itself cannot be written the keychain forgets the new key and the account runs plain that
time. Each later start seals whatever it finds plain, and checks it all again only while
not `done`. Attachments are sealed afterwards in the background, each
checked against its BLAKE3 id before it takes the plain one's place, with no copy. The
copies of the direct chats in `before-sealing/` go as soon as sealing is `done` (they would
keep in the clear, for as long as the application runs, messages that disappear meanwhile);
the others (the identity, the friends file, the MLS state), and all of them when sealing was
cut short, at a later start than the one that found it `done`, which opens the folder whole
with the key the keychain gives again. While sealing is not `done`, or an attachment is
still plain, the settings and the bell at start say that not all of this computer's data is
encrypted yet (`at_rest` is `Unfinished`). A keychain that gives the key back once and then loses it (reset,
or an item tied to the program's signature) leaves the folder refusing to open, and the
account still in those copies.

A plain secret (the identity, the friends file or its copy, the recovery key, an MLS state)
found in a folder that has a key is sealed only while sealing is under way as a note sealed
with that key says (`sealing.json` is sealed from 0.1.0-beta.8 on): anyone who can write the
folder could have put it there. Otherwise it is refused: moved, as it is, to `refused/`,
never read nor sealed. Two cases are the account's own though. A folder of which neither the
identity nor the friends file is sealed yet is sealed whole (its recovery key refused: copies
are turned on again). And a folder whose sealing an earlier version (noting it in the clear)
cut short after the identity, which `before-sealing/identity.key` shows (a plain copy of the
identity sealed there): sealing goes on, taking a plain secret as its copy in
`before-sealing/` holds it, or as it is when sealing never got to copy it (a recovery key only
as its copy holds it), and refusing one that differs. Whoever could write such a folder could
read the account's identity in it in the clear anyway, until the start after sealing ended
removes the copies. 0.1.0-beta.8 refused those files; a later start takes them back from
`refused/` by the same rule, sealed, where nothing was put since, while the identity's copy
is still there. Refused files are never kept without a word: each start says where they are
(a notice), a move to another computer writes them over and removes them with the other
secrets (5e), and a folder whose friends file was refused, or damaged with its copy (both set
aside as `friends.bin.damaged…`), shows the welcome page with a note of where they are kept
and that nothing was deleted (`kept_profile`); making a new profile there is asked first, and
never writes over them. Has a profile
(`has_profile`) does not ask the keychain, so the first window opens at once, unless the
folder has a key and its friends file is plain or was refused; the keychain is otherwise
asked when the core starts. Keys and decrypted secrets are wiped from memory when dropped
where the types allow (`zeroize`), and so are the password and the random scalars of a run
of a friend code or a move code: the `spake2` crate (0.4) that runs them is a copy of its own
for that (`vendor/spake2`, see its FOGATA.md).

Network findings carry a `Topic` besides the English text, so a screen can word them in its
own language.

**Layout of the data folder.** A file `layout` in the data folder holds the number of the
layout the build that last ran in it writes (`store::LAYOUT`, 1 now; a folder without it is
of layout 1, from before there was one). It is written as a core starts, never by opening a
folder only to look or to restore into it. A folder whose layout is above the build's own is
refused with a message to install the newer version again, rather than misread: an older
build put over a newer one by hand (`drive/` installs; the updater never goes back). The
number goes up only when an older build would misread what a newer one writes (a file moved,
or written in another form). New files that older builds leave alone (`levels.json`) do not
raise it, nor do the per-file fallbacks (`.bak`, `.damaged`) by which each file is read with
care. A move to another computer (5e) does not carry the marker: the new computer writes its
own.

## 5d. The desktop application (app/, fogata-ffi)

A Flutter application on top of the core. `crates/fogata-ffi` is the bridge
(flutter_rust_bridge 2.13): it starts the core once on its own Tokio runtime and gives the
screens plain values (ids as hex strings, times in milliseconds) and one stream of events.
That runtime has one worker per core, at most four; the bridge's calls run on it too, the
blocking ones on its pool for blocking work (`crates/fogata-ffi/src/bridge.rs`, in place of
the bridge's default handler, which started a runtime and a pool of threads of its own, each
as large as the computer has cores).
`flutter_rust_bridge_codegen generate` (run in `app/`) writes `app/lib/src/rust/` and
`crates/fogata-ffi/src/frb_generated.rs` from `crates/fogata-ffi/src/api/`.

The window's state (`app/lib/src/app_state.dart`) is one `AppState` that does what the
person asks and follows the core's events, holding its data in parts (`app/lib/src/state/`),
each a `ChangeNotifier` told only of its own changes: `Prefs` (what `app.json` keeps, the
banner's picture and the animations), `ToastBoard`, `Notices` (the bell, missed calls,
declined invitations; it reads friends, requests and invitations through `NoticeSources`),
`CallState` (the room's `Call` and the direct calls) with `LiveCall` (what the call says as
it goes: someone connects, mutes, shares), `RoomState` (the room shown, what the window
remembers of rooms, previews, files waiting to go), `Contacts` (the profile, groups,
members, friends, requests, how each member is reached) and `Navigation` (the page, the
group selected, what floats over the window), besides `DirectChats`. Who is talking
(`Call.talking`) and who is typing (`AppState.typingChanged`) are told apart again.
`AppScope` gives the state to the widgets: `AppScope.of` builds a widget again on any
change (all parts but the toasts), `AppScope.only(context, {...})` and `AppScope.watch<T>`
only on changes of the parts named, and `AppScope.read` never. The room page, the side
column, the rail, the settings, the direct chats and the toasts depend only on the parts
they show: a line in a room's chat no longer builds the side column or the settings, a
toast builds only the toasts, a direct chat's change builds nothing of a room, and what the
call says as it goes builds the room's people and controls (and the column's rows of
people) but not its chat (`test/rebuilds_test.dart` counts it). `AppState.notifyListeners`
still tells every part, for the changes that touch many of them (the groups or friends
read again, a call starting or ending).

The screens follow the design canvas, in dark and light, laid out like a chat application
people already know: a rail on the left with home on top and every group as an icon (with a
badge while its room is live), a column beside it for what is selected (for home: the
overview, friends and settings; for a group: its members and invite, then its voice rooms
with who is in them), the page in the centre and, in a room, the chat and files beside it,
each in a tab of their own. The person's own pill sits at the bottom left, under both the rail and the
column, with the microphone, deafen and the settings (in the rail instead when the window
leaves no room for the column). Beside the microphone and deafen there, and beside the
microphone, deafen and camera in a room's controls, a small arrow opens that device's panel
attached to the button (over it, the arrow pointing up meanwhile): the devices of its kind
to choose from, the system's default first, noise suppression under the microphones, and the
way to Voice & video in the settings. A choice applies at once, also in a call, and is kept.
The list follows devices plugged in or out while it is open (read again every 4 s, as in the
settings). The names the system's default also goes by in WebRTC's lists ("default: " and the
device's name on Linux, "default" and "communications" on Windows, "default" on macOS) and
nameless entries (macOS lists one for the default input of a computer without any) are left
out there and in the settings, since "System default" is that; one chosen before counts as
"System default".
Clicking the pill opens their panel: the top in a
colour or picture chosen in the settings (friends see it on the card too), picture with
status, name, bio, edit profile (it opens My profile), Online / Offline (appear offline; away comes by itself, 3.9) and a button that copies a new friend
link. The pages: welcome (what Fogata is for, then make the identity), home
(invitations from friends, live rooms first, then the rest), new group (an icon, or a
picture from the computer, set as the group's picture record right after `Create`), friends (QR, link or
code, requests, presence, a Call button for each friend online, missed calls, and a
menu on each friend's "···" to add them to a group or remove them, which asks first), a group's members (in their order, how each is reached; managers
invite with a link; each moves up or down, times out and removes those below; anyone leaves,
the first too),
settings and a group's room (people, chat with files and who came and went,
files with progress, "connecting", "only you" before closing a room, and "could not
connect"). A direct call (3.11) shows as that room with its own title, "Calling …" with
rings while it rings, a card when a friend rings (a small one in the corner while in a
room, saying that answering leaves it), why a call did not go through, and missed calls in
a corner note (it goes by itself after 8 seconds, or with a click, like a toast) and in
Friends and the bell. A friend in the same voice room as this device is not rung: the
Call buttons say "You're both in" that room instead. The interface speaks English, Spanish,
Brazilian Portuguese, French, German, Italian, Russian, Japanese and Simplified Chinese, the
system's language at first (`docs/translating.md`). Links, QR codes, short codes and invitations are
blurred until an eye button shows them for ten seconds, and a copy button copies them
without showing, so nothing leaks from a window on a shared screen.

The settings float over the window: a column lists the parts under
two headings, User (My profile: picture, name, bio, the card's background and this
computer's fingerprint) and App (Appearance: theme; Language & time: the language, each
named in itself, applied at once and kept as `uiLanguage` in `app.json` once chosen (until
then the system's), and the time format; Voice & video: the
microphone with noise suppression and the speaker, then the camera in a box of its own with
Test video, a preview only this device shows: it opens the camera, as a call does, only
when pressed, and closes it when the part or the settings close, the window is hidden or
minimized, or a call opens the camera; and Mirror my camera (on by default, kept as
`mirrorCamera`), which draws this device's own camera as in a mirror wherever it shows here,
while the others always get its picture as it is. Test microphone opens the chosen
microphone as a call does and shows its level: WebRTC records, and tells the level, only
while a connection sends, so the track goes from one connection to another in the same
process and the sending one's statistics say how loud it is; nothing leaves the computer.
Hear myself (off until turned on) plays it through the chosen speaker. The test closes when
the part or the settings close, the window is hidden, or a call starts, which stops it
before anything else; alone in a call, the own tile's level comes from such a loop too;
Sounds; Behaviour: opening Fogata when the computer starts, off by default and read from
the system rather than kept by Fogata (an entry in `$XDG_CONFIG_HOME/autostart`, by
default `~/.config/autostart`, on Linux; inside a Flatpak, the Background portal's
`RequestBackground` with `autostart`, through the runner's `fogata/desktop` channel, with
the sandbox's own entry remembering its answer; the application as a login item through
`SMAppService` on macOS 13 and later; a `Fogata` value under the user's
`Software\Microsoft\Windows\CurrentVersion\Run` key on Windows), and keeping running in
the tray, where there is one; Network: whether this
device relays for others, and the network check; Storage & downloads: the downloads folder,
what direct messages take and the data folder), and the rest shows the chosen part. Where the
two do not fit side by side (narrower than 680 points), the list and a part take turns, with
a button back to the list. Closing the settings (Esc, a click beside them, the close button,
or going to another place) while My profile holds changes not saved asks first: save them,
discard them or keep editing. The settings and the other pages that scroll as a whole glide
with each step of a mouse wheel (200 ms, the steps adding up while the wheel turns), as
browsers do, rather than jump; a touchpad's small steps move them as they come. A friend's card (Friends, and the right side of a direct chat)
shows the friend's banner, picture, name, code, status in words and bio, then Call and
Message side by side (one under the other when their words would not fit so), and the
friends in common.

The window has six themes, each one set of the same colour tokens (`Fg` in theme.dart): Dark
(the default), Midnight (true black), Dim (softer greys), Ember (warm greys), Contrast (black
and white with stronger lines and colours) and Light, plus System, which follows the computer
between Dark and Light. Every theme's text colours read at 4.5:1 or more on its surfaces
(checked by test/theme_test.dart). The choice is kept as `theme` in `app.json`; versions
from before the extra themes read any name but `light` and `system` as Dark.

Text follows the system's text size up to 1.5 times (`maxTextScale` in shell.dart); larger
settings stop there, the most the window at its smallest (960 by 640) holds. Rows that hold
text keep their height as a minimum and grow with it rather than cut it.

Every button can be reached with Tab (with a blue ring) and used with Enter or Space. Tab
goes through the title bar, the rail, the column and the page in turn; while the settings, a
group's settings, the person's panel, a friend's card, the bell, a device panel or a menu
float over the page, Tab stays inside them and Esc closes them (a device panel or a menu
gives the focus back to the button that opened it). The panel, the card and the bell open
beside the button that opened them and, like device panels and menus, stay beside it when
the window changes size (placed again from where the button is then); the bell's
(380 points wide, 340 in a window under 1000) hangs under the bell with their right edges
lined up, inside the window, its notices scrolling when it is short. Device panels and
menus open over or under their button, wherever there is more room, and always inside the
window: moved in from its edges and, when taller than the room, scrolling.

Ctrl K (⌘K on macOS), or the round button in the title bar (the shortcut in its tooltip),
opens the quick switcher from any page: in the middle of the window (as tall as it opens:
typing shortens its list from the bottom and the field stays put), a field and the direct
chats, friends, voice rooms and groups whose names match it (a voice room also by its group's
name). With nothing typed it lists the places shown lately in this session, then the chats
newest first, the voice rooms with people in them, the groups and the other friends; typed,
names that start with the text come first, then those with a word that does. Arrows choose,
Enter goes there (a voice room asks the same questions as the side column before leaving a
call or closing a room) and Esc closes it. Friends filters its list with its own field.

Both message boxes (a direct chat and a room's chat) have a smiley button that opens the
emoji picker above it: a search by name, the skin tone, the groups as icons to jump to, the
recently used first and the emoji in a grid, with the name of the one under the pointer or
chosen. The arrows choose from the search field, Enter or a click puts the emoji at the
box's cursor and closes it (with Shift it stays open), and Esc closes it. It stays inside
the chat's column (a room's chat beside the call takes 7 emoji to a line instead of 8 to
fit) and the window, and it closes, as the ":name" suggestions do, when the chat hides
(by hand, or a window made smaller) or goes offstage. Typing ":name" in
a box offers the matching emoji above it (Enter or Tab takes one, Esc leaves the text), and a
whole ":name:" that is an emoji's name or a common short name ("joy", "+1") becomes it. The
names come from Unicode's emoji-test.txt (`app/tool/emoji_table.py` writes
`lib/src/emoji_table.dart`), up to Emoji 13.0 so every system's emoji font has them; they
are drawn with the system's colour font (Apple Color Emoji, Segoe UI Emoji, or Noto Color
Emoji on Linux, where the picker says when fontconfig has no colour emoji font). Country
flags are left out on Windows, whose font draws them as two letters. The interface is set in
Inter (SF Pro on macOS), headings (page titles, the group's name over its column, the voice
rooms' names) included, in bold (`theme.dart`'s `hx`); only the wordmark, "Fogata", is in the
app's own face, Fogata Ember (`wordmark`, `assets/fonts/FogataEmber-Heavy.ttf`). The recently used
emoji and the skin tone are kept in `app.json`. The search also matches the names and
keywords of the language in use, then English's (`assets/emoji/<code>.tsv`, from Unicode
CLDR, read the first time the picker or `:name` needs them, away from the window's thread;
see docs/translating.md).

A group a friend added this device to arrives as an invitation on the home page: nothing
of it runs, is announced or is sent until the person joins it. Declining forgets its log and
is final: friends in the group offer its log again each time they meet this device, and
`groups.json` keeps the decline (the group id and the entries that had added this device;
older versions keep such a record as one they cannot read), so those offers are ignored.
The device also signs a `Leave` on the log it was offered and hands it to the members that
are its friends, at once to those online and later to whoever offers the group again; once
they take it, the group no longer counts the device, stops offering it the group, and an
manager can add it again. Only such a new addition (an `Add` or `Join` of this device the
decline did not see) brings the invitation back. Older versions take the `Leave` like any
entry a member hands them.
The `Leave` goes in `FriendRequest::Group` with `declined`, the hash of the
entry that made the invitation (the latest `Add` of the device, or its `Join`). The member
that made it (the author of the `Add`, or for a `Join` the manager who wrote the invite)
tells its person "<name> declined the invitation
to <group>" in the bell and a toast (`InvitationDeclined`), once per invitation while it
runs; the other members only take the `Leave`. It takes the request also from a device no
longer a member, when another member brought the `Leave` first. Older versions ignore the
field, and older decliners send none, so their declines still show nothing. An inviter that
was away when the person declined hears it when it next offers the group to them, unless
another member brought it the `Leave` before.
At most 8 invitations from one friend and 32 in all wait at once (further ones are refused
until the person answers some), an offered log holds at most 4096 entries and 4 MiB, and
the bell announces at most three invitations per friend a minute; the others still show in
the list.
Group and chat ids are random, so the same id twice was chosen on purpose: a friend's
invitation or chat under the id of a group being joined with an invite is refused, a chat
that already has that id gives it up (its messages stay; the next one makes a new chat
group), and the log the manager sends replaces an invitation kept under that id. A decline
holds only for the group declined, told by its first entry: a friend that offers a made-up
log under a real group's id first does not keep the real group out once the person declines
the made-up one.

Direct messages (3.13) list under Home and Friends in the column, newest first, each with
the friend's picture and presence, the last line and the messages unread; the rail's home
badge counts them too, the bell shows a chat with unread messages, and someone else's
message makes the room's soft pop unless that chat is being read. Friends and a friend's
card open a chat with Message. A chat's page has the friend's name and status, the
disappearing timer as a pill that opens the four choices, and the call button (off while the
friend is offline); the messages by day and by person (one name for the messages a person
writes within five minutes), the timer's changes as lines, pictures, videos (their first
picture, darker until fetched) and files with what can be done with each, and under this
device's last message whether it was delivered or waits for the friend; the box to write
(with + for files and the emoji picker); and on the right the friend's card with what the chat takes here and
"Free up space…", which opens the settings at Storage & downloads. Once the two are no longer friends
the box gives way to a line and Delete chat. Settings › Storage & downloads shows what direct messages
take, the room for their pictures, videos and files (a smaller one than what they take asks
first), what comes by itself, the timer of new chats, and each chat's Clear files.

The room shows a tile per person (a green ring and bars while they talk, a crossed
microphone when muted, their camera when it is on) and a wider tile per shared screen. The
controls turn the microphone, camera and screen on and off, pick a screen or window to
share, open the device choices, hide or show the chat and leave. Files picked with the
chat's clip or dropped anywhere on the room (`desktop_drop` 0.8.0, our copy in
`app/third_party/desktop_drop`: later versions depend on the MPL `dbus` package) wait above
the message box, each with its size and the folder it is in, a small picture for images and
a button to take it out; only files of this computer go: on Linux a drop is taken only as a
list of files (`text/uri-list`, its `file:` entries), never as text that reads as a path, and
a path that is not absolute, a device or a pipe is left out (the core refuses to share
anything but a regular file, too). Any other target a browser or a file manager offers
(`text/x-moz-url`, `_NETSCAPE_URL`, `x-special/gnome-copied-files`, plain text) adds nothing.
A web page can start a drag whose list names any file of the computer, whatever the browser,
so a dropped file that is hidden (a name that starts with a dot), in a private place (a macOS
`Library` outside iCloud Drive and the File Provider folders in `CloudStorage`, Windows
`AppData`, the system folders of any drive or share, Linux `/run` outside the portal's
documents and `/media`) or a link that leads to one is never added by dragging: the rest is,
and a notice says how to share such a file on purpose, picked with the clip, which no page
can do. Names are read as the system reads them: `..` worked out, without their case on macOS
and Windows (the long s and the Kelvin sign as s and k), Windows' trailing dots and spaces
and its short names (`APPDAT~1`), and where each really is, links followed. The user's
Library is looked for under the account's own folder as well as under `HOME`, which macOS's
App Sandbox points at the application's container. Inside a Flatpak, files dropped from
outside the sandbox arrive as the document portal's paths (`/run/user/<id>/doc/...`), and
it is the portal that decides what the application may read: only what the person dragged
is exported. A file waiting or being sent is not added again; files that fail wait again without
changing the tab, and a send stops when the room is left. Files picked with the file picker do not ask. Send (or Enter) shares them with the message
typed, if any, which goes first, and Esc takes them all out. Folders are left out, and
said so. A file that follows its sender's message by a few seconds shows under it. A right
click (or a long press) on a person, on their tile or in the room's list, opens their
voice volume. On the groups page a
live room's card has the same microphone and camera buttons for entering.

A watched screen, or a camera enlarged with a click, fills the room in its own shape with
the others in a strip under it. The chat and files beside a room or call are hidden and
shown again by hand with the chat button among the controls (a direct call's Chat or
Files in the side column shows them too); hidden, they keep what was typed. On a wide window the choice is
kept in the app's settings (`roomChat` in `app.json`) for every room and call, and
nothing hides them by itself. A small window (a room page under 832 points, where the
chat beside the call would leave the controls less than their whole row: a window under
about 1144 points, 24 more before it counts as wide again) starts every call with them
hidden; shown there by hand, or with Chat or Files, they stay shown for that call however
the window changes, until hidden by hand. Shown on a small window they are shown on wide
ones too, and hidden on a wide one they are hidden on small ones too. While another page is
shown, or the settings cover the room, someone else's screen or camera floats over the
window instead (`screens/pip.dart`): in the bottom right corner above the corner's cards,
with whose it is and, under the pointer, a button that stops watching. It can be dragged
anywhere under the header and stays inside the window as the window changes size (pushed
back into the corner, it returns to its place above the cards); a click goes back to the
room or call, where it fills the room again. It goes when the screen or camera ends, the
person leaves, or the room or call does. It draws the renderer the room draws, so nothing
more is received. Toasts keep clear of the corner: between the side column and the
corner's cards, or above them when that space is narrow. They show over everything else,
so a note said from inside the settings, a group's settings or a dialog is seen there.
Clicks go through them, except on one about a file ("Saved to …"): it stays 10 s, with
Show in folder and Copy path. The microphone
stays as it was left in the last call; the camera starts off every time. macOS asks for the
microphone and the camera the first time they are used (`Info.plist` and the entitlements
explain why).

Sounds for the room this device is in: a fourth going up when this device
or someone else comes in, the same fourth going down when someone leaves, two soft low notes
a third apart when someone else writes (at most one per 0.8 s), a quick step down when this device mutes
and up when it unmutes, three notes up when anyone starts sharing a camera, a screen or a
window and down when they stop, and two light high notes when someone starts or stops
watching a screen. Around the rooms: a two-note chime when the bell gets something new (a
missed call, a friend request, someone asking to join, a group a friend added this device
to, this identity open on another computer too; at most one every two seconds, nothing for
what was waiting at start), a fall to the low tonic when a direct call ends or does not go
through, and the rings of a direct call (a short song on a soft felt piano, one bar of
3.2 s, while a friend calls, a slow soft rise up the chord while this device calls), which
sound even with the sounds off. The player itself starts a ring again every 3.2 s, each bar
over the tail of the one before and on the beat however long the first took to make, until
told to stop: answering, declining or hanging up stops it at once, before the core is done,
faded out over 30 ms while other sounds play on. The rings take about a second to make, so
they are made ahead on a thread of their own when the core starts (at 48 kHz, and at the
output's rate once it opens at another). Nothing sounds for
the room's past arriving in the first three seconds after entering, nor for files. The
sounds are synthesized in
`crates/fogata-ffi/src/sounds.rs` as one family (a soft mallet on a glassy bar, and a warmer
FM mallet for the start of sharing, mostly with a light stereo chorus, a short room and a
gentle EQ, in A major pentatonic, each brought to about the same loudness and kept under
-1 dBFS; each also has other variants to compare by ear, and one table there picks the one
each plays). They are made in stereo, an output with one channel getting both sides mixed, and
played with cpal on the output chosen in Settings (the speaker the calls use; the system's
default while it is not connected); Settings turns them off (`app.json`). One thread
plays them, mixing sounds that overlap and keeping the output open for fifteen seconds after
the last one, so sounds close together do not each open the device; an output that stopped
(its device went away) is opened again for the next sound, and each different failure to
play is a warning in the log the first time it happens. On Linux they go through ALSA to
PipeWire, which needs ALSA's plugin for the sound server on the computer (`pipewire-alsa`,
or PulseAudio's ALSA plugin under PulseAudio): without it ALSA's default output is the sound
card itself, and the sounds fail while a call holds it. Before the first sound the player
looks for that plugin among ALSA's devices and, missing, says so in the log, naming the
package. The runner names that
stream "Fogata" (`PIPEWIRE_ALSA`), so the system's volume controls show it, and keep its
volume, as Fogata's, and sends it wherever `PULSE_SINK` sends the rest of the
application's sound. There the chosen speaker is reached through the sound server
instead: flutter_webrtc's output keeper (`fogataSoundsTo`) moves every stream of this
process (the sounds, the chat videos, the call) to the chosen output as it appears. Chat
videos play on the chosen output too: on macOS the player is given it
(`audioOutputDeviceUniqueID`), and on Windows the media engine is given its endpoint
(`IMFMediaEngineAudioEndpointId`; the voice engine lists outputs by endpoint id, or by name
when there is none), when it is connected as the video opens; otherwise they play on the
system's default.

Voice messages in direct chats (3.13) are recorded and played in the Rust library
(`crates/fogata-ffi/src/voice/`, files by `crates/fogata-voice`). The microphone button
beside the message box starts recording at one click; the box becomes the recording bar
(Delete throws the recording away without asking, and so does Esc up to 5 seconds, after
which the line under the bar stops offering it so a key pressed by habit never throws away
minutes of talk; also while the microphone is still
opening: once open it closes again; not while it is being sent, when it is on its way; Send
or Enter sends it), with the
time, the last minute counted down from 14:00, and the voice's bars as they come. One under
half a second is not sent: Send throws it away and a toast says it was too short. At 15
minutes recording stops by itself and waits for Send or Delete; so does a recording whose
chat is left (even while its microphone opens), whose friend goes offline, whose window is
hidden in the tray or minimized (nobody sees the recording bar there), or when a call starts; while it waits, the button in
another chat starts nothing and says to send or delete that one first. It cannot start in a direct
call or a voice room, where the call has the microphone. The friend is not told that
anything is being recorded. Recording uses the microphone chosen in Settings › Voice &
video. On Linux the voice engine names it by its description in the sound server; its side
looks up the server's own name for it (`fogataInputName`, FOGATA.md), which ALSA's plugins
for the sound server open (`pulse:DEVICE=…`, then `pipewire:NODE=…`), then ALSA's default,
then cpal's. On Windows and macOS the voice engine names it by the system's id (endpoint id,
Core Audio uid), and recording takes the cpal input of that id, or of that name, else the
system's default input. When the chosen microphone is not the one that opened (not there, or
not found), the line under the recording bar says so: the default microphone records. On macOS it asks for the
same permission calls use. The sound is made mono at 48 kHz, cleaned of noise with the
calls' suppressor when that is on, and encoded at once: Opus for voice at about 24 kbit/s
(constrained variable rate), 20 ms a packet: a minute takes 130 to 175 kB, 15 minutes
1.9 to 2.6 MB. A microphone that does not open, gives nothing for two seconds, or (on macOS,
where one the system has not allowed records digital silence) gives only silence, shows a
note over the box with a way to Settings › Voice & video. One that stops giving sound part way
(an error, or nothing more for two seconds) stops the recording, which is kept to send, with a
note over the recording bar that says so. What the window asks for every 80 ms (where
the recording stands), and stopping or throwing it away, never wait: sending waits up to
300 ms for the microphone to close, so all it took is in, without holding the recording.
`fogata-voice` writes the Ogg
pages itself and reads a friend's file strictly: every page's checksum, its stream, its
place and flags, packets no longer than 8 kB and of a length their first byte allows, at
most 4 MB and 15 minutes and a second in all, nothing after the last page; anything else
does not play. One message plays at a time, decoded as it plays, at 1×, 1.5× or 2× (the
pill on the message), faster ones stretched in time by WSOLA (20 ms pieces, each taken
within 5 ms of its place where it best continues the one before), so the voice keeps its
pitch. It plays with cpal on the chosen output, as the sounds do, with a buffer of about
40 ms, so pausing and the time shown follow at once; the output closes four seconds after
nothing plays. The output's callback decodes holding the player alone, with room made
beforehand for all it keeps; where the message is and whether it plays are kept apart, so
what the screen reads every 80 ms never waits for the decoding. Speed and place are
forgotten when the chat closes. A message that is deleted or whose timer runs out while it
plays or is paused stops, also when newer messages pushed it out of what the chat has read
(the core is asked about it).

What the window keeps of a room (who came and left, files whose holders left, when each
chat line arrived, which is the order the chat is shown in) lives only in memory, like the
room, and is bounded: the last 1000 notes (with the files they show) and the arrival of the
last 1000 chat lines. Messages, in rooms and direct chats, are drawn without the marks that
set the direction of what follows (embeddings, overrides, isolates: one can make
`photo\u{202E}gnp.exe` read as `photoexe.png`) and with at most 4 marks on one character (a
few are accents, hundreds smear over other lines); a line in a script written right to left
is set apart in its own direction. Only the drawing changes: they are kept and sent as
written. The animations that run for as long as something is shown (a live room's dot, someone
talking) step twenty times a second in their own layer, and the room's page is rebuilt when
its state changes, not every second; so an open room does not keep the window redrawing. The only file the application adds to the data folder is `app.json`
with the chosen theme, whether sounds are on, whether the microphone is on, and the chosen
devices; the core writes it, readable by the owner only like the other files there. It also holds
whether this device relays other members' traffic (`bridge`, on by default; each relayed
circuit is capped at 2 GB), which the core reads when it starts. "Readable by the owner
only" is a mode on Linux and macOS (folders 0700, files 0600); Windows has no such modes,
and the data folder, in the user's profile, and what is in it keep the permissions they
inherit from that profile, which lets in only that user, the system and administrators.
The window's other files (the banner's picture and the animations, the note left for a
restart, problem reports, its own log) are written through the core the same way
(`write_private`: owner only, through a temporary file, a link where it goes replaced
rather than followed), and the data folder and its `window` folder are made private before
the window opens its lock file there. A field of `app.json` that cannot be read (a setting
that is on or off and is anything but true or false included) keeps its default; the file
is then kept as `app.json.bad` and the bell says so once.

The app raises its limit of open files and sockets at start (to the system's maximum, at
most 10240): desktop apps on macOS get 256. The network keeps about 50 open in normal use
(DHT nodes are reached over QUIC and their connections close soon, see 3.2); the higher
limit leaves room for calls, files and bursts of TCP connections.

The window has no system title bar: the application's header is the title bar (dragging
its empty parts moves the window, a double click maximizes it). On Windows and Linux it ends
with Fogata's own minimize, maximize/restore and close buttons; on macOS the content reaches
the top edge and the system's traffic lights sit in the header, centred by an empty unified
toolbar (`macos/Runner/MainFlutterWindow.swift`), with room left for them except in full
screen. On macOS release builds run in the App Sandbox (network and user-chosen files and
Downloads allowed) and with the hardened runtime (no JIT, no debugger, only libraries signed
by the app's own team: `tools/sign-macos.sh` signs every framework, see `docs/releasing.md`);
development and test builds run outside the sandbox so tests can use their own folders. On
Windows the runner's first step limits where libraries are looked for to the application's
folder and System32 (`SetDefaultDllDirectories`), so a library left in the current folder or
on PATH is never loaded. Release builds open the Rust library by its own name only; debug
builds also look for it in the source tree, for tests. Linux keeps GTK's client-side
frame (shadow and resize edges) by giving the window a hidden header bar; Windows hides its
caption through window_manager at start. On Linux under Wayland with a fractional scale
(125 %, 150 %) the window looks slightly blurry: GTK 3, which Flutter's Linux embedder runs
on, only gives it a whole scale, so it is drawn at the next whole scale and the compositor
shrinks it. Whole scales (100 %, 200 %) and X11 are sharp. A known limit of Flutter on
Linux, not something the app can set.

On Linux quitting destroys the window, and with it Flutter's view, while the engine still
runs: by itself the engine shuts down a moment later, when its last reference goes, from
the main loop. Two faults of Flutter's Linux embedder made that quit crash at times. The
view frees the compositor that frames are drawn into when its widgets go, so a frame the
engine's raster thread was presenting at that moment aborted the application (GLib refuses
to clear the compositor's locked mutex) or finished in freed memory. And the engine's
platform task runner can leave a timer behind (one posted from another thread while an
earlier one ran), which ran on the freed task runner at the main loop's next turn. The
runner (`linux/runner/my_application.cc`) keeps the view's widgets and the engine from the
window's destruction: the engine shuts down once the main loop has ended (the runner's
application object goes), its frame finished and its threads stopped, and the widgets go
after it. Windows takes the view from the engine, waiting for its frame, before destroying
it, and macOS shuts the engine down as the application terminates, its window still there.

The bridge library is built with `cargo` from the workspace on every desktop system
(`app/rust_builder/linux/CMakeLists.txt`, `windows/CMakeLists.txt`, and on macOS
`macos/build_rust.sh`, the pod's script phase, which joins one static library per
architecture with `lipo`), with `--locked`: the crates are the ones `Cargo.lock` names and
the Rust the one `rust-toolchain.toml` names, and nothing is fetched or run for the build
itself (cargokit, kept for mobile, ran a Dart tool whose packages came from pub.dev on every
clean build). libopus (opusic-sys) is built with CMake: on Windows the build passes its own
CMake (Visual Studio's, often not on the `PATH`) as `CMAKE`, which the `cmake` crate uses,
with the Ninja beside it as the generator (a Visual Studio project built from inside the
app's build, itself one, fails in CMake's compiler checks with MSB4018); on macOS the script looks for it on the `PATH`, then in Homebrew's and CMake.app's places.
On Windows everything Rust builds (the bridge library, the command-line program) has
Control Flow Guard, as the runner and the plugins have (`/guard:cf`): `.cargo/config.toml`
adds `-C control-flow-guard` for MSVC targets. Cargo runs from the workspace, where it finds that file. On Linux and Windows
the build runs cargo through `app/rust_builder/cargo_build.cmake`, which looks at the build's
environment when cargo starts and leaves out, and names in the build's output, every
variable that would change the code cargo makes or what makes it: `RUSTFLAGS` (which would
replace those flags), `CARGO_TARGET_*`, `CARGO_BUILD_*` (but the number of jobs),
`CARGO_PROFILE_RELEASE_*`, `RUSTUP_TOOLCHAIN`, the compiler and flag variables that reach
libopus and AWS-LC (`CC`, `CFLAGS`, `CFLAGS_<target>`, `TARGET_CFLAGS`...), MSVC's `CL` and
`LINK`, and `CMAKE_*`. `tools/beta/release.py` then refuses a Windows build in which any
`.exe` or `.dll` lacks ASLR, DEP, Control Flow Guard or EH continuation metadata
(`/guard:ehcont`), except what each cannot have: the bridge library has no EH continuation
metadata (no stable rustc option makes it), nor does `libwebrtc.dll`, built with WebRTC's
own toolchain, without C++ exceptions, and `flutter_windows.dll`, Flutter's engine as its
upstream release ships it, has neither. The runner and the plugins have both on x64. CET shadow stack
compatibility is off on purpose: Windows turns shadow stacks on for a process when its
program says it is compatible, Fogata was never tried on a processor that has them, and a
mistake there would end it at start on every such computer, which is worse than going
without that protection. The runner and the plugins are linked with `/CETCOMPAT:NO`, so a
linker whose default changes does not turn it on, and `tools/beta/release.py` refuses a
`fogata.exe` that is compatible. `libwebrtc.dll` comes compatible from its own build, which
changes nothing while the program is not. It is to be turned back on once Fogata has been
tried on such a processor. On Linux the runner is a position-independent executable
(`-fPIE -pie`).

**The splash.** Every start opens a small window first
(`app/lib/src/splash.dart`): 300 by 360, no frame, a dark card (round corners on Linux,
where its corners are see-through when the desktop composites windows; macOS and Windows
round the window themselves) with the animated logo (`assets/splash/`, twice the size for
screens of high density) and what is happening under it: starting, connecting (the core
starts), checking for updates (friends, members and the releases page are asked; the splash
waits up to six seconds for a newer release, less once those asked said there is none),
downloading (a bar, how much, from whom), installing and restarting. A release that is
ready, or whose download moves, is installed there and Fogata starts again as the new
version; a download that stops for 15 s, or that would take more than three minutes at its
pace so far (unless this copy must update), goes on behind the main window. A release that
becomes ready while the main window shows is announced once in a toast, and the title bar's
pill (green, amber when this copy must update) restarts into it. The splash shows at least 0.6 s. The runners make the
window the splash before anything shows (Linux in `my_application.cc`, macOS in
`MainFlutterWindow.swift`, Windows in `setUpWindow`, around the middle of the window made
for the main one), and `becomeMainWindow` (`window.dart`) gives it back its frame, window
buttons, smallest size (960 by 640, or the screen's work area if that is smaller, so the
window's bottom is never kept under the taskbar or the Dock) and 1440 by 900 (or the
screen's work area) in the middle of the screen (`mainWindowSizes`); on Linux through the channel `fogata/desktop` (`mainWindow`), which
hides the window while it changes and shows it again, so that on Wayland, where a window
cannot place itself, the compositor places it as a new one. (GTK sizes a window that
cannot be resized by what its contents ask, so the Linux splash is resizable, only with no
frame to do it from.)

**Updates while running.** When a newer release is ready the title bar shows a small green
pill, "Update ready · 0.1.1", with Restart, which installs it and quits for the new version
to start; amber when this copy must update to keep talking to that release. No dialog.

**Problem reports.** Settings › Report a problem: the person writes what happened and,
unless they untick it, adds the log of the last two days. The core and the window keep that
log in `diagnostics/` of the data folder (`crates/fogata-ffi/src/logs.rs`): one file per
day (UTC), `fogata-YYYY-MM-DD.log`, with what Fogata's crates say at `info` and above and
the libraries' warnings, at most 8 MiB a day, today's and the two days before kept;
`app-YYYY-MM-DD.log` beside it holds the window's errors (kept as long). The folder is its user's only
(0700, its files 0600 on Linux and macOS; on Windows the data folder's own permissions) and
never moves with the account (5e). What is personal never reaches it: each line is
scrubbed as it is formatted (`logs/scrub.rs`). The time, level, target and span names of a
line are fixed in the code and stay; a field keeps its value only when the value is a
number, a yes or no, a duration or one word of the code (an enum's variant), an address or
a peer keeps its shape with marks for its values (`/ip4/‹ip›/udp/‹port›/quic-v1/p2p/…k3A`),
an error's text is scrubbed like a message, and anything else becomes `‹value›`. A
message (its template and what was put in it cannot be told apart once formatted) loses
text in quotes of any kind (messages, whatever a library printed of data); the names of
people, groups, rooms and members known to the core, read again after its events, and the
names of the computer and its user; the paths of the data and home folders; the lines the
core writes with a name in them; links and Fogata invitations, e-mail addresses, IP
addresses and ports (plain and in multiaddrs), host names of the local network, paths and
file names (with the words of a name before its last), codes (a move's typed without its
dash too), keys (16 bytes of base64 too), long runs of hex (with or without `0x`), base32 or
base58 and lists of bytes, all
replaced by marks (`‹name›`, `‹ip›`…); peer ids keep their last six characters, device keys
and other long hex their first six, fingerprints their short form, so the steps of one
device can still be followed. Fogata's own crates log ids, never names. Standard error,
which a desktop session may keep in its journal, is scrubbed the same way at any level
`RUST_LOG` asks for; only a debug build with `FOGATA_RAW_LOGS` set leaves the values as they
are. A panic of the Rust side goes to the log as a scrubbed line, and standard error is
told only where in the code it happened (`logs::report_panics`), not what it said. A panic
in a call of the screens (from the reading of its arguments, which the bridge does before
the call runs, to the writing of its result), or in a task one waits for, reaches the window
with what it said scrubbed the same way (`bridge.rs`), since the window may print it; so does
any text of the core's for the window (why a download failed, a note, an update's or a
move's failure) that tells of a task's panic, however it was put into words (`bridge::told`).
The core's own errors for a task that panicked say only that (`fogata_net::task_failed`), and
the window says of any of them only that something went wrong (`explain`). The
window scrubs its errors before writing them. In a release build its console takes no
message either (`app/lib/src/console.dart`): an error the window did not catch (Flutter's,
or one that reached the root zone, which the engine would print whole) writes one line of its
kind and the place in the code, and goes to the day's log scrubbed; what the window and its
libraries print with `debugPrint` (the video library's `print`s too, the fork's `FOGATA.md`)
goes to the day's log alone. A debug build prints as Flutter does. A report is a
zip (`app/lib/src/report.dart`, at most 5 MB, the oldest lines going first): `report.txt`
(what was written, version and build, system and its version, language) and the log files
of the last 48 hours, scrubbed again line by line with the same rules and the names the
window knows, a second pass for anything a rule of the first missed in an older file
(`test/report_test.dart`). "Email it" writes the zip into the downloads folder,
opens the mail application (`mailto:` to the team's address, with a subject and the file's
name: what was written goes only in the file, since the address is handed to another
program on its command line) and shows the file in its folder to attach; "Save to a file"
writes it where the person chooses. Nothing is sent by Fogata itself. The application opens
only `https:` and `mailto:` addresses in other programs (`openExternal`). Settings › Donate to Fogata opens the
donation page in the browser (`kofiUrl` in `screens/donate.dart`).

## 5e. Moving an account to another computer (fogata-core `transfer`)

The person shows a code on the new computer ("Bring your account from another computer" on
the welcome page, or Settings → Move account) and types it on the old one (Settings → Move
account → Move to another computer). The identity, the profile and friends, every group
with its log and MLS state, the direct chats with their attachments and the settings go
straight from one to the other, encrypted, and the old computer stops using the identity, so
there is never a second copy of the person. The CLI does the same with `fogata transfer
receive` and `fogata transfer send CODE` (and `status`, `forget`, `keep`, `discard`).

In the application the core stops for the move and starts again after it, in the same process
(`stop_core`, then `start`, which waits for the folder's lock): on the old computer when the
code is typed, and again if the move does not happen; on the new one only when it replaces a
profile, which after the move restarts the application as the account that came. The old
computer then shows only that the account moved (with "Delete Fogata's data on this
computer"), or, while the new one has not confirmed, a field for the code that finishes the
move and, behind a warning, a way to keep using the account there; a new computer with an
account that came whole but unfinished shows the code that finishes it.

**The code** (`fogata-friends` `transfer.rs`): ten Crockford base32 characters,
`XXXXX-XXXXX`, nine random and a check character (values weighted 1, 3, ... 17, summed modulo
32), so a mistyped code is told at once ("that isn't a whole code"); every single wrong
character is caught. It lasts 10 minutes. The first four characters (20 bits) are the
*nameplate*: the rendezvous secret comes from them alone, so a DHT node that sees the
rendezvous point can learn those four characters and nothing about the rest. All nine are
the password of a SPAKE2 run (Ed25519 group), so nothing seen on the network lets anyone test
guesses of the code offline: the rendezvous point, the run's messages and the encrypted
transfer carry nothing to test a guess against. Guessing is online only, one guess per run,
and the old computer proves the code first (`/fogata/transfer/2`):
- the old computer sends the nameplate and its SPAKE2 message; the new one answers with its
  own SPAKE2 message alone, which tells nothing of the code to whoever sent the start; the
  old one sends its proof, and only once that checked out does the new one send its own;
- the old computer proves the code typed to **at most three computers**, one proof each:
  every computer that answers under the code's nameplate with a SPAKE2 message learns from
  that proof whether its own guess was right, so each proof is a guess given away. Nothing
  else tells the computer showing the code apart from one that learnt the nameplate (the
  rendezvous point, the announcement sealed under the nameplate, a connection that lists
  the protocol: each is had for the nameplate; a key in the announcement too), so the old
  computer runs the code with every computer it finds, side by side (at most eight at
  once, those on the local network first), and one that answered and did not prove the code
  back is never tried again and does not end the move: a stranger that answers first spends
  one of the three, not the move. The move stops, having sent nothing, once the three proofs
  went, or once nothing more is being or is to be tried and 15 seconds passed since the old
  computer began to look (the computer showing the code may be found a little after a
  stranger): with "the other computer did not accept the code" when one computer answered
  (a code mistyped, or a stranger alone), with "several computers answered this code and
  none of them knew it" (`others_answered`) when more did, which a code mistyped never
  causes, so the person can tell someone trying to stop the move from a typo. A start begins
  with a proof of the nameplate, so a computer on the same network showing another code
  says so without a proof being used;
- the new computer counts only a wrong proof, a guess that missed, and stops answering the
  code after three, showing a new one. A run that stops before its proof (a stranger that
  learnt the nameplate and stalls, or goes once it has the answer) learnt nothing and costs
  the code nothing: under the first way, where the new computer proved first in its answer,
  such a run had to count, so three of them ended any code whose nameplate was found. The
  new computer answers runs at once, each in a task of its own up to the proof it brings
  (5 s for the start, 10 s for the proof): one per computer, its newest stream in place of
  the one before, and at most eight, the oldest stopped for a new one; so a stranger that
  stalls keeps neither the real run nor the person's Cancel nor the code's end waiting,
  and to keep the real run out it must bring new computers faster than that run ends. The
  proofs are checked one at a time, in the order they come, so runs side by side give no
  more guesses than runs one after the other.
A guesser therefore gets a handful of tries at 2^25 for the part it cannot see: three on
the old computer per code typed and three on the new one per code shown, each one in
33,554,432, so at most six in 33 million, about one in 5.6 million, per code. The
application does not try a code that did not match, or that several computers answered, a
second time either. Someone who can bring three computers that answer under the nameplate
before the one showing the code is reached can still keep a move from happening (the
budget is there because the code is short); it learns nothing, and the person is told that
several computers answered.

**Versions.** The first way (`/fogata/transfer/1`, versions up to 0.1.0-beta.7) and this one
do not run with each other, and each side tells the other apart before anything is tried or
sent, so neither is left half moved. The new computer also takes streams of the first way:
a start there that names its code is left unanswered, and the older old computer tries
again every five seconds, its code's try unspent, until it gives up after three minutes
("could not reach each other"); after three such starts the new computer stops the move
with "update both computers to the same version" (`Failure::OldVersion`, `old_version` to
the screens). One start alone does not stop it: anyone who learnt the nameplate can send
one, and the old computer of this version may be on its way. Whoever learnt the nameplate
can end a code with three, as it can with three wrong proofs; it learns nothing. A start
for another code is told so. The old computer, finding a computer that takes only the
first way, sends it the nameplate and a SPAKE2 message that is none (`old_protocol_probe`):
a computer of that way showing another code says so, and one showing this code drops the
stream without counting a try (it cannot run the message), which a stranger can do as
well; so this does not stop the search at once, but when nothing else took the move by
the time the search ends (as above) it stops with the same words; the older new computer
keeps showing its code until it runs out.

The nameplate's 20 bits are a trade-off: the code stays ten characters, and every bit given
to the rendezvous point is taken from the 25 the guesser must find. 2^20 rendezvous points per
6-hour window can be watched by someone with the means (about 50 DHT lookups a second): that
watcher learns the address of computers showing a code at that moment, and can try to answer
first. It still gets one guess per computer that answers, three at most per code typed (each
one in 33 million); a miss is not tried again, and the move goes on with the computer
showing the code when that one is found, so it can only spoil moves by bringing several
computers, not take an account. A computer on the
local network is tried first, and one found over the DHT only when none is there after five
seconds, so a move between two computers at the same place does not reach a watcher at all. The run binds both transport identities (authenticated by the transport) and
both SPAKE2 messages into a transcript, from which HKDF-SHA256 makes one key for each
direction and a confirmation for each side; the old computer confirms first, and the new
one only after checking it, so a computer that starts a run with the nameplate and a guess
learns nothing from the new one but "no", and only by sending its proof; one that answers
the old computer with a guess learns from its proof whether that guess was right, and no
more, and cannot prove it back. The person on the old computer then sees the new computer's name and what
moves, and says yes; the person on the new computer sees the old computer's name and what
comes (and the profile it replaces there), and says yes too. Each computer tells the other
its yes (`Go`), and nothing is sent until both did; a no on either ends the move with nothing
changed.

**Meeting.** Each side runs a node of its own for the move, with a new transport identity and
no friend or group, so nobody sees it come online meanwhile: the old computer's own node is
stopped first (the application quits its core), so nothing in the folder changes while it is
read and the MLS state and logs move at the epochs they are at. Both announce under the
rendezvous secret (6-hour keys as for groups) and look it up at every punch slot (10 s); what
they find is dialled at once and at the punch slots, as members are (3.4), and on one local
network mDNS finds it anyway. Their local addresses go in the announcements sealed (3.3) under
a key made from the nameplate alone, so whoever watches the rendezvous point can read them
(a key from the rest of the code, however slow to make, would let it test guesses of the code
offline against them), and each opens and dials the other's: two routers at home with one public
address and no way back in, where the device on the main router lets only its own port in, are
crossed by the one behind the inner router (lab: `transfer-two-routers`, both ways). Each side
listens on the port its folder keeps (the new computer picks the one the account will keep
there when its folder has none), so a firewall rule for it serves the move too. The new
computer takes streams of `/fogata/transfer/2` (and of `/1`, see Versions above); the old one
opens one to each connected peer that says it takes it, and, only while none does, to each it found
under the code itself: the new computer lists the protocol only to computers it found (or on
its local network), so one finding the other is enough
(`a_move_needs_only_one_computer_to_find_the_other`). A computer that only connected to the
old one and lists the protocol is tried too: refusing it would gain nothing (a stranger can
as well announce itself under the rendezvous point, for the nameplate) and would lose a move
where only the new computer found the other. One of the code's proofs is spent only on a
computer that takes the protocol and runs the exchange: one that does not refuses the stream
before anything is said. Two NATs that cannot be punched (one that
picks a new port per destination) do not meet: after three minutes the old computer says the
two could not reach each other, and the screens suggest the same network. A member of the
person's groups does not carry a move: it would have to let a device that is no member through
its relay (later).

**What moves.** After the run every frame is encrypted with XChaCha20-Poly1305 under the
direction's key, numbered by its nonce, so a frame changed, dropped, repeated or put out of
order makes the move stop. The old computer sends a summary (the account's signed profile,
how many friends, groups, chats and attachments, how many bytes), then, after both yeses, every
file of the folder in the form a folder without a key keeps it: a sealed file is opened with
the old folder's key, attachments as they are read. A copy of a damaged friends file, journal
or pins is opened as its form says it was sealed: for `damaged` (a plain copy sealing sealed
later), or for what it was when it was put aside already sealed (`friends`; a chain of the
chat's, read up to its damage); one that opens as none of those (damaged in its box too)
stays behind, said in the log, rather than stop the move or a copy (5g). Each file goes as its path, its pieces
(256 KiB) and the BLAKE3 of its contents, checked at the end. The new computer decides from
the path alone what a file is and how to seal it (the identity, the friends file, the MLS
states and the friends' profiles whole, a chat's journal and pins as chains, attachments as
streams, copies of damaged ones whole for `damaged`, the rest as they are), takes only paths of those kinds (no `..`, no root, at most four
deep, never twice), and writes everything into `incoming/` of its folder (how far the move
got is kept beside it in `incoming.json`, where nothing the old computer sends can land), sealed with a new
data key that its own keychain keeps (plain only where the keychain keeps none, as for a new
profile). Not moved: what belongs to the computer (`sealed.json`, the locks, `before-sealing/`,
videos fetched to play, downloads cut short, the log in `diagnostics/`, the new versions of
Fogata kept in `updates/` (made for this computer's system, up to hundreds of MB), how chats end as
written with the old key, which the new folder works out again, and what was judged of the group
logs, sealed with the old key, judged again there (5.1)); in `app.json` the microphone, speakers and camera chosen,
and in `meta.json` the port, which the new computer keeps its own of.

**The end, with never two copies.**
1. The new computer checks `incoming/`: the identity opens and is the one in the summary, the
   profile is its own, every group's log reads. It says `Staged`.
2. The old computer writes `moved.json` ("moving"): from then on its store refuses to make or
   read the identity and the core does not start. It says `Commit`.
3. The new computer records the commit and puts `incoming/` in place: its folder's own files
   go to `replaced/` (but the locks of the copy running there, `running.lock` and `window/`),
   the new ones come out. Each step can be taken again, and a computer that stops in the
   middle goes on at its next start. This work holds the folder's lock itself and always runs
   to its end: throwing a staged move away while it runs finishes it instead. It says
   `Active`, with a keyed hash of the move's id and of this run's own secret under the move's
   secret: a key from the run that sent the account, which both computers keep (in
   `moved.json` and `incoming.json`, then `moved-in.json`) and only the computer the account
   went to can use. A later run that finishes the move tells the move's id to whatever
   computer shows the code typed, which cannot prove anything with it. It says so as soon as the
   account is in place, or once the commit is recorded when putting it in place has to wait
   for the next start (a full disk, say); only then does `replaced/` go. The keychain key named
   in it (a profile replaced from Settings, after a warning) goes as soon as its files are
   aside, and again with `replaced/` if the keychain could not be reached then. If the commit cannot even
   be recorded, it says so and the move stays staged. The old computer waits up to five
   minutes for `Active`.
4. The old computer checks that hash, marks itself "moved" without the move's secret, says
   `Moved` (the new computer then empties the secret where it keeps it; without that word it
   keeps it for a week, to finish the move with an old computer still waiting, and empties
   it at the first start after) and deletes the identity,
   the friends file and its copies, the MLS state, the tickets of invites waiting for a
   manager (`joins.json`) and any plain copies left from sealing. Chats, files and logs stay, sealed,
   until the person deletes them (`forget`: everything, and the folder's key in the keychain;
   Fogata then starts there like new).

Stopping a move: before the code checked out it simply goes; after, it stops where nothing is
half done, on either computer and also while files come, the other one told why (the new
computer throws `incoming/` away with the keychain key it made, before it lets go of the
folder); once step 1 or 2 began it is not
stopped any more, and the screens say it is too late. A failure after the old computer let
go says that the account is put in place at the next start, not that nothing changed. While a
move waits to be finished, no profile is made in the folder.

Bounds on the new computer: the disk must hold what is announced (with room to spare; the
application asks the system, `statvfs` on Linux and macOS and `GetDiskFreeSpaceExW` on
Windows, in fogata-ffi's `disk.rs`; the command line asks `statvfs` too, in its own `disk.rs`,
and elsewhere a full disk stops its move instead; an offer above 1 TiB is refused outright), at most the
bytes announced and 64 MiB more come, at most 500,000 files, no empty piece, and the whole
within ten minutes and a pace of 32 KiB/s. Names are ASCII letters, digits, `.`, `_` and `-`
(an attachment's ending after its id excepted), never start with a dot, and are never one of
the move's own (`incoming.json`, `moved.json`, `sealed.json`, the locks...); a path is refused
when a disk without cases would take it for one of those or for a sealed kind
(`IDENTITY.KEY`), when it could be a short form Windows gives another name (`INCOMI~1.JSO`), or
when Windows cannot keep it (`:`, a trailing dot or space, device names such as `CON` or
`COM1`); the old computer leaves such files where they are. Attachments are read as the store reads them, and
other files as they are, whatever they start with.

A move cut short before step 2 changes nothing: the new computer throws `incoming/` away (at
once, or at its next start). When only its `Staged` was lost, it keeps `incoming/` as staged
while the old computer goes on with the account; a code shown there again and typed on the
old one moves the account anew, in place of what was staged (an offer of another account
is refused while something is staged). Cut between 2 and 3 (the very last message lost), the old
computer stays "moving" and the new one keeps `incoming/` as staged: showing a code there
again and typing it on the old one finishes the move (`Resume`, by the move's random id,
which only the two know; the old computer names that computer and asks again first). Only
the person can make the old computer use the account again
then (`keep_this_computer`, behind a warning), or make the new one throw it away. When the
new computer's `Active` is what was lost, it runs the account and keeps the move's secret in
`moved-in.json`; a code shown there (Settings, bring an account here) and typed on the old
one finishes the move the same way, within the week the secret is kept.

In the application the move belongs to the whole window, not to the settings' page: its
question shows over whatever page is open, and its end brings the core back (or shows that
the account moved) also when the page was left. The command line asks before `receive`
replaces a profile and before `keep`, and `send` without a code reads it from its input, out
of the shell's history and the list of processes.

**Threat model.**
- A DHT node or anyone on the path sees a rendezvous point and encrypted traffic between two
  transport identities; it can learn the nameplate, and with it open the local addresses
  and port in the two announcements. Nothing it sees allows a test of the rest of the code.
- Someone who announces under the rendezvous point (a DHT node near it, a device on the same
  network) can try one guess per computer it answers the old computer with, three at most
  per code typed, and three against the new one, each a 2^-25 chance. One computer that
  answers first no longer stops the move; three that answer before the computer showing the
  code is reached do, with "several computers answered" (a nuisance, not a leak). It can
  also hold some of the new computer's runs (see above), which only delays the move while it
  keeps bringing new computers.
- A frame changed or replayed on the way stops the move; nothing is applied.
- A malicious new computer that won the code gets the account: that is what the code is for,
  and why the old computer names it and asks first. A malicious old computer that won it
  can fill the new folder with an account of its choice (only inside `incoming/`, and the
  person sees whose account it is before it starts); it cannot write outside the folder.
- The old computer keeps chats and attachments, sealed with its own key, until the person
  deletes them; the identity, the friend secrets, the recovery key, the MLS states, the plain
  copies made while sealing and the files sealing refused are written over and gone from it.

## 5f. Updates of pre-releases (fogata-update, fogata-net `updates`, fogata-core `updates`)

Pre-releases (the `beta` channel) travel over Fogata itself, from friend to friend, with no
server. A malicious update would reach every tester, so a copy takes
only what the release key signed.

**The release.** A manifest (JSON) names the product and channel (`dev.holypanda.fogata`,
`beta`), a build number that goes up with every release, the version people see, the oldest
build that still works with it (`min_build`: below it a copy is told it must update, for a
change of the network protocol), when it was made and when it expires (180 days by
default), notes, and one package per platform (`linux-flatpak`, `windows-x64`,
`macos-arm64`, `macos-x64`) with its file name, size, BLAKE3 and SHA-256, and the BLAKE3 of
every 1 MiB chunk. The universal Mac app is one package listed under both Mac platforms (the
same file, size and hashes), as a copy takes the platform of the process it runs as. The release key (Ed25519) signs `fogata/v1/release-manifest\0` followed by
the manifest's exact bytes; the public keys a build trusts are compiled into it
(`RELEASE_KEYS`, read at run time from the keys marker of the library, which the release
tooling checks in every package together with its build and the commit it was built from),
never read from the data folder or a package. A copy checks size (256 KiB
at most), key and signature before it parses anything, then every field: a format it knows,
plain names, a package of at most 400 MB, chunk hashes that match the size. It takes a
release only when its build is above every build it runs or has (no downgrade, no replay)
and at most 1000 above the build it runs (`MAX_BUILD_STEP`), it was made no earlier than the
release it holds, it has not expired, and it was not made more than a day after the copy's
clock. The secret
half of the key stays in `~/.config/fogata-release/` on the publisher's computer (made by
`fogata release keygen`, readable by its owner only, and sealed with a passphrase: Argon2id
with a random salt, 128 MiB and three passes, then XChaCha20-Poly1305 under the label
`fogata/v1/release-key`, so a copy of the file alone is no key; `--plain` leaves it in hex
for a computer kept offline). Every build trusts two keys or none (a test checks it): a
release key, and a rescue key made the same way (`keygen --rescue`) on a drive kept offline.
A release the rescue key signs may be up to 2000 builds above the build a copy runs and is
not held to the time of the release held, so when a release key leaks or is lost, a release
numbered 1001 above the last good build, signed with the rescue key, is taken over anything
the leaked key can have made a copy hold (at most 1000 above its build), and the build in it
trusts a new release key in place of the old; nothing the leaked key signs passes it after.
Rotating a key that did not leak is a release that adds the new public key, then one that
drops the old. There is no revocation list: a copy that installed a release of a leaked key
runs that code. The first installs, the files of a pre-release, are listed with their SHA-256 in
`SHA256SUMS`, signed by the same key as an SSH signature (namespace `file`: `SSHSIG`, the
namespace and the file's SHA-512, a message no manifest signature covers), which `fogata
release verify-sums` (which also fails on a file the list does not name) and `ssh-keygen -Y
verify -I fogata-prerelease` check (the pre-release's keys go by that name in allowed signers
files, the SSH key of public releases by `fogata-release`, so neither passes for the other);
testers check that signature with the key lines of `tools/allowed_signers`, taken from the
source code apart from the download, and compare the checksum with their file's, since the
builds have no paid code signing.

**Spreading** (`/fogata/update/1`). A device asks each friend (recognised on its connection)
and each verified member of a shared group that lists the protocol, when it connects and
every 20 minutes, for a release newer than the build it has, naming its platform; the answer
is the manifest with its key and signature and the platforms whose packages that device
holds complete. Anyone else's stream is closed before anything is read from it and nothing
about releases goes to the DHT, so only friends and members learn a device's build. A newer, good release is kept
in `updates/` (`release.json`, `release.sig`) and the package for this platform fetched from
a device that has it, from the first missing chunk to the end; each chunk is checked against
its hash as it arrives and written to `updates/<platform>.pack.part` (made new, owner only,
never through a link), so a damaged chunk ends the download at once, keeping the good ones,
and that device is not asked again for an hour. A download that stops goes on from its last
whole chunk. The whole package is hashed again (BLAKE3 and SHA-256) before it becomes
`<platform>.pack`. Then the device tells its friends and members it has the build (`HAVE`,
which only makes them ask it, at most every 30 seconds) and gives the release on: so it
spreads to a friend of a friend who is not the publisher's friend. A device fetches only its
own platform's package, and another platform's only when a friend or member asked it for
that one (it then fetches it from whoever has it, one at a time), so a Windows friend behind
a Linux friend gets it too; it keeps one such package at most (one nobody asked for in an
hour makes way for another), lets it go once nobody has asked for it in two days, and
fetches it only while its own and it stay under 600 MiB together, so a friend cannot fill
its disk with every platform's. A package starts to come only once the disk has room for
what of it is still to come and 256 MiB to spare, as a download does; until then the
update's status says the disk is why, and it is tried again at the next step. The publisher's copy (`fogata release publish` marks it)
keeps every platform's package. A device uploads to three devices at once at most, one stream
per device (a friend or member may connect with several sessions, counted together), 4 MB/s
in all, and 1 GiB to one device in a day (a package whose rest does not fit in the day is
not started: the device is told to ask later and goes on from where it stopped); asks from one device are answered at most every 3
seconds, and so are its words that it has a build and its fetches of a package that is not
there. A friend or member has at most 4 requests answered at once, and all together 32; a
stream past that is closed unread, and a request that has not arrived whole 10 s after its
stream came is closed too. An upload whose reader took less than 64 KiB/s over a minute of
waiting for it (time spent on the disk or the 4 MB/s pace not counted, as for files above:
`slow.rs`) ends while another device turned away for want of a place in the last two minutes
may be waiting (the device cut off goes on from its last whole chunk when it asks again, 45 s
later); a slow line keeps its place while nobody waits. An ask, a word that a device has a
build, or a fetch whose stream ends or is reset before any answer is made again, after 0.1,
0.3 and 1 s, as a file request is: friends that hear of a release at once all ask its
holder at once, and copies before 0.1.0-beta.9 drop all but one of such a burst (see the
`stream` behaviour). The publisher
puts a release into a data folder of his (`fogata release publish`, which checks every byte
first and refuses a release its own build would let go at its next start: older, too far
above, expired), and the copy running there gives it like any other.

**From the releases page.** Besides friends and members, a copy reads the newest releases of
the project's repository on GitHub (`RELEASES_API`, `RELEASES_REPO` in `fogata-update`; the
API's list of releases, drafts left out) soon after it starts, when the splash asks, and every
six hours (every fifteen minutes after the page could not be read): of the first three that
carry `release.json` and `release.sig` it fetches those two files (256 KiB and 1 KiB at most,
4 MiB for the list) and takes the newest by exactly the rules above, as from a friend: the
page decides nothing a friend could not. Its package for this platform is the asset with the
file name the manifest gives, fetched over HTTPS (rustls, the system's certificate
authorities) into the same `<platform>.pack.part`, each 1 MiB chunk checked against its hash
as it arrives, a stopped download going on from the last whole chunk with a range request;
a damaged chunk ends it, and that release is fetched from friends and members after that.
A download that brings less than 16 KiB/s over two minutes, or nothing for one, stops and
goes on two minutes later; the small files must arrive within five minutes.
The page is used before friends and members when it has the package, which spares their
uploads; a package from there is given on like any other. A copy that trusts no release key
does not read the page (nothing could pass), nor do the network lab's copies
(`FOGATA_UPDATE_WEB=off`, which `fogata update run --web` also takes, or another API's address
and repository, for tests). GitHub learns the device's address, that it runs Fogata (the
`User-Agent` names the version) and when it looks: the only server Fogata talks to of its own.

Only the newest release is kept, with its packages; one older than the running build goes at
start. Nothing installs without the application asking `install_update`: at start, while
its splash shows, when a release is ready or comes while it waits (5d, The splash: a few
seconds for friends and the releases page to answer, then the download as long as it moves,
three minutes at most unless this copy must update);
later, when the person presses Restart in the title bar's pill. The state the screens read (`update_status`): up to date,
checking, downloading (bytes), verifying, ready, installing, failed (no package for this
platform, disk, cannot install here, install failed, an install that did not take) and
whether this copy must update, and while it downloads, from which friend's or member's
device. `fogata update status|run|install` show and do the same from the command line.

**Installing.** The package is hashed once more, unpacked (the package format refuses
absolute paths, `..`, names that differ only in case or that Windows reads otherwise, links
that lead out, and writes into a folder made new, owner only, never through a link), then:

- *Linux (Flatpak)*: the package is an OSTree repository holding the new version, its
  commit and summary signed (GnuPG, an Ed25519 key of its own made once by
  `tools/beta/flatpak-key.sh`, under a passphrase GnuPG asks for, in a folder of its own
  apart from the release and rescue keys: the scripts refuse a GnuPG home in a folder that
  holds one of them, and `fogata release keygen` a folder that holds it). The first install
  (`flatpak install --user Fogata.flatpak`) carries that key's public half and names
  `http://127.0.0.1:47811/` as the application's origin, which flatpak then verifies
  (`gpg-verify`, `gpg-verify-summary`): from that address it takes only a summary and a
  commit this key signed, and no commit older than the one installed (it compares their
  times; `tools/beta/test-flatpak-origin.sh` tries each case). Any program of the computer
  can answer on that fixed loopback port while Fogata is not serving there (another
  account, a Flatpak with the network), and `flatpak update` or a software centre then
  fetches from it; without the key it can offer only versions this key signed that are
  newer than the one installed, which flatpak installs although Fogata itself might not
  (one whose release expired, say). Flatpak knows nothing of the release key, the manifest
  or its build rules, and installs through the portal whatever the remote's newest commit
  is (the portal names no commit): it is this key, not the release key, that decides what
  flatpak installs on Linux. A commit it signs installs with no manifest at all, and a
  summary it signs can make flatpak trust further keys (`xa.gpg-keys`, added to the old,
  never in place of them) or fetch from another address (`xa.redirect-url`), for good. To
  install, the application serves the unpacked repository there, on the
  loopback interface only and to requests that name that address, first asks the port for
  the repository's `config` to see that its own server, and no other program, answers, and
  then asks the Flatpak portal to update it (`UpdateMonitor.Update`). The portal asks the
  person once whether Fogata may update itself, checks the application id and refuses a
  version that asks for more permissions, so the sandbox stays whole (the alternative,
  `flatpak-spawn --host` with `--talk-name=org.freedesktop.Flatpak`, would let any bug in
  the application run anything on the computer). The new version is started through the
  portal (`Spawn`, newest version, no arguments) and this one quits. Other programs that
  update Flatpaks find nothing at that address while no update installs, which flatpak
  treats as a warning for that application only.
- *Windows*: the application lives in `%LOCALAPPDATA%\Programs\Fogata` (installed for one
  person, no administrator, by the first install's script, which copies only the files
  `Fogata.sha256` in the download lists, checks each copy's SHA-256 and clears the mark of
  the web of those files alone). The package is unpacked beside
  it as `Fogata.new`; a hidden PowerShell waits for the application to end, moves the old
  folder aside, puts the new one in its place and starts it, or puts the old one back and
  starts it if a step fails. Files Fogata writes itself carry no mark of the web, so
  SmartScreen is not shown again (the manifest's signature stands for the files instead).
- *macOS*: the sandboxed application cannot replace its own bundle, so it unpacks the new
  `Fogata.app` into `updates/` in its container and opens
  `Contents/Library/Helpers/Fogata Updater.app`, signed by the same team without the
  sandbox. A sandboxed application cannot pass arguments to an application it opens (Launch
  Services drops them), so the helper finds both itself: the new bundle at the one place Fogata
  unpacks it, and Fogata among the running copies of the installed bundle; tests run outside
  the sandbox may name both as arguments, checked the same way. Anyone can open the helper with
  arguments, so it believes them only as far as it checks them: the installed bundle is
  the one it is inside of; the new bundle must be at `.../updates/Fogata.app` inside
  Fogata's container (`~/Library/Containers/dev.holypanda.fogata/Data`, pinned in at build
  time; the path as given must be the path resolved, so no link leads elsewhere); the
  process must be running and be the installed bundle's. It waits for Fogata to end (Fogata
  stays two seconds after opening it), copies the new bundle to a folder of its own on the
  installed bundle's volume that nothing else can write, and checks that copy
  (`app/macos/Updater/Checks.swift`): a valid signature, for `dev.holypanda.fogata`, by the
  team pinned in at build time (`tools/beta/macos-updater.sh --team`; so the permissions
  given to Fogata stay), with the App Sandbox entitlement (so no debug or profile build of
  the team, which run outside it), and a build, as the Info.plist the signature seals says
  (`release.py` checks it is the release's), above the installed bundle's and at most 2000
  (`MAX_RESCUE_STEP`) above it. Fogata applies the release's build rules before it
  downloads anything, but whatever can write its container, Fogata itself included, could
  put an older bundle of the team there: the helper refuses it on its own. Then it
  clears the quarantine of that copy, swaps it in and opens it; on any failure after the
  copy it removes the new bundle and opens the old one, and on bad arguments it touches
  nothing. The copy's own folder is made readable by every user (Fogata unpacked it
  readable by its user only) before the swap, so every user of the Mac can open it.
  Signatures carry Apple's timestamp (`tools/sign-macos.sh`), so they stay valid
  when the Apple Development certificate expires after its year.

Each leaves `window/restart` so the new copy waits for the old one to let go of the data
folder, and `updates/installed.json`: a copy that starts still older than the build it
installed says so (the install did not take) rather than offering it again. The data folder
is never touched; later pre-releases open the data of earlier ones as every version does.

## 5g. Copies that restore an account without the keychain (fogata-core `recovery`)

The data folder is sealed with a key the system keychain keeps (6, `vault`). A keychain that
is lost (reset, the program's signature changed, the user deleted) or a disk that dies takes
the account with it, and a move (5e) needs both computers alive at once. A recovery copy is a
file the person keeps where they like, which only a key they hold opens. The CLI has `fogata
recovery on`, `export FILE [--attachments]`, `size`, `restore FILE [--replace]
[--unencrypted]`, `status` and `forget-replaced`; the key is read from the input, never from
the command line. `restore` shows whose account the copy holds and what the folder holds, and
asks before it replaces anything; though the command line opens folders without sealing them,
what it restores is sealed with a new key of the system keychain, as an account that moves in
is (`--unencrypted` keeps it plain where no keychain keeps a key, for that restore only:
nothing in the folder says so if it does not happen). It refuses a copy the disk has no room
for, as its move does.

**The recovery key** (`fogata-crypto` `recovery.rs`). Turning copies on makes an X25519 pair
(`NewKey`). The secret half is the recovery key: 32 random bytes, shown once as 52 Crockford
base32 characters in 13 groups of four and a check character (weighted as the move's code is,
so every single wrong character is told at once), wiped from memory after and never written
anywhere. It is taken (`turn_on`) only once the person typed two of its groups back; until
then copies stay for the key they were for. The folder then keeps `recovery.bin`: the public
half and an *auth key*, HKDF-SHA256 of the secret half (`fogata/v1/recovery-auth`), sealed
with the folder's data key as the identity is (and sealed again by a move, which takes it
along). Making a copy needs only what the folder keeps, so it can be made at any time; only
restoring asks for the key. Being random and 256 bits long, the key needs no slow hashing,
which a password the person chose would (and would still be weak against an attack on a
stolen copy). Turning copies on again makes a new pair: copies made before open only with the
old key. A short name of the key (`XXXX XXXX`, from its public half) is shown when it is made
and with every copy, to compare.

**What a copy holds** is what a move moves, with its list and exclusions (5e: never
`sealed.json`, `sealing.json`, `running.lock` nor `before-sealing/`), each file in the form a
folder without a key keeps it: the identity, the profile and friends, every group with its log
and MLS state, the direct chats, the settings and `recovery.bin`. The attachments of chats
only when the person asks (`size` says what they would add first). A copy is made with no core
running on the folder, as a move is, so the files read are the folder at one moment.

**The file** (`Fogata-YYYY-MM-DD.fogata-copy`):
- a header in the clear, 63 bytes: `FOGATACOPY`, version 1, the first 16 bytes of
  SHA-256(`fogata/v1/recovery-fingerprint` ‖ public key), the public half of a pair made for
  this copy alone, and the day it was made (days since 1970, big-endian). Nothing of the
  account: no name, profile, sizes nor count of files. The fingerprint lets the screens say
  "this copy is for another key" before anything is tried;
- the copy's key: HKDF-SHA256 of X25519(that pair, the public half), salted with the whole
  header, info `fogata/v1/recovery-copy`;
- frames: each its length (4 bytes, big-endian) and XChaCha20-Poly1305 under that key,
  numbered by its nonce, with the whole header as associated data. A frame holds a kind byte
  and a message in JSON or a piece of a file (256 KiB, as in a move). The first, at most
  1 KiB, is the copy's proof: HMAC-SHA256 of the header under the auth key. The header holds
  the public half of a pair made for that copy alone, whose secret only the computer that made
  it knew, so the proof cannot be moved to another copy, and only a computer that holds the
  account (or the recovery key) can make one. It is checked before anything larger is read.
  The second is the list: every file with its size and BLAKE3, the account's device key and
  name, and the version of Fogata that wrote it. Then each file in that order (its path, its pieces, its end), and last an end
  that counts the files, after which nothing may follow. A byte of the header or of a frame
  changed, a frame dropped, repeated or out of order, a copy cut short or with something after
  its end: none opens;
- it is written beside its name, under a new name of its own (`NAME.<random>.tmp`, never a
  file or link already there), and takes the name only when whole, so a copy cut short never
  looks like one and an older copy under that name stays as it was. The application writes
  it straight into the file chosen instead (`export_into`), since the macOS sandbox lets it
  write that file alone after a save dialog: never through a link there (`O_NOFOLLOW`), for
  this user alone, removed if the copy fails; one cut short all the same lacks the end every
  copy closes with, and is refused as damaged. A copy is not made inside
  the data folder (it would go into the next copy and with a move), nor restored from inside
  the folder it goes into (a restore empties it).

**Restoring** (`restore`, the welcome page's "Restore from a copy"). `open_copy` says
first, with the key, whose account a copy holds (its name and device fingerprint) and
`account_in` what the folder holds, for the person to compare. The copy opens with the key,
its proof checks, and its list is checked (names a folder takes, as in a move; each once; no more
files nor bytes than a move takes, added up without overflowing; room for it on the disk, when
the application can tell, as for a move) before anything in the folder changes. Then it goes
in as a move's account does, with the same record (`incoming.json`): every file is written into
`incoming/`, sealed with a new key of this computer's keychain (plain only where a new profile
could be), its size and BLAKE3 checked against the list; then the identity must open and be
the one the list names, the profile must be its own and signed by it, and every group's log
must read. Any failure until then throws `incoming/` away with the key made for it, and the
folder is as it was. Then the record says `commit` and the files are put in place as in a move
(the folder's own aside to `replaced/`, the new ones out): a computer that stops in the middle
finishes it at the next start, and one that stopped before throws it away, never leaving some
of each (tested by making every write fail in turn). A folder that holds anything of an account
(the identity, a profile even damaged, groups, MLS states, chats or the copies sealing left:
anything a move takes but the settings a new folder starts with; not a data key alone, which
the application makes for a new folder the first time it opens it, before any account, nor a
device key alone, which the welcome page makes to show its fingerprint and nobody knows yet:
that one is replaced unasked and deleted with the folder's key, not kept aside) is
refused
unless the person says to replace it, after a warning with its name; that account is then kept
whole in `replaced/` with its keychain key (`restored.json` says so), through every start,
until the person deletes it (`forget_replaced`). While it is kept, another restore or a move
into the folder is refused rather than throw it away, and so is deleting what is left of an
account that moved away (`forget`). Neither `replaced/` nor `restored.json`
moves nor goes into a copy. A version of Fogata from before copies does not know
`restored.json`, and going back to one removes the kept account at its first start. A `recovery.bin`
that does not open keeps a folder from being sealed whole, as any secret does, until the
person sets it aside; set aside, it goes neither with a move nor into a copy, and copies are
turned on again.

**In the application** (fogata-ffi `recovery`, `screens/recovery.dart`). Settings › Storage
says whether copies are on and for which key, and turns them on or changes the key: the key
is made in Rust and held in memory only, never on the disk, until the person typed two of its
groups back (`recovery_turn_on` takes it by its short name; a key shown and not taken is
dropped). Making a copy offers the files sent in chats (off, with their size), asks where with
the system's save dialog (never in the data folder), and stops the core while it is made, as a
move does, starting it again after, whatever happened; deleting the account a restore replaced
does the same. The work that stopped the core owns the stop (`AppState.stoppedBy`): only it
starts the core again, and neither a move nor another copy begins meanwhile.
The welcome page's "Restore from a copy" picks the file (`inspect`: its day, and whether it is
for another key than the folder's), asks the key with a live check, shows whose account it
holds with the warning to restore only when no other computer uses it (with a way to move it
instead) and what the folder holds that it replaces, then restores with the keychain the
application chose; where that keychain cannot keep a key, it offers the welcome page's choice
to go on unencrypted and restores again with `unencrypted`. An account an earlier restore kept
aside is offered for deletion right there, since Settings cannot be reached without a profile;
a copy checked whole that could not be put in place is finished by starting again, as a move's
is. The key typed leaves its field once restored or when the card is left. The refusals and a copy's problems
come typed (`UiRestoreRefusal`, `UiCopyProblem`, `UiKeyProblem`), so the screens word them in
the person's language. Once restored, the application starts with the account, as after a
move.

**The same identity.** A restore brings back the same device key, as a move does: the device
key is the identity (1), friends know it by it and groups have it in their members and MLS
credentials, so a new key would be another person to all of them. A copy restored while
another computer still uses the account makes two of the same person; the screens say to
restore only when no other computer has it (and to move it otherwise), and two live sessions of
one identity are told already (4.3). A copy made before the group went on to a new epoch comes
back behind it: the log a member's sync brings has the new epochs' secrets sealed to the same
device, so it reads and writes in the group again by itself (tested). With MLS (5.5, not used
yet), a state put back follows the Commits it missed from where it was, unless the device
itself committed after the copy (its weekly update, an add or a removal): then the state put
back sees its own key write a link it did not make and is out of the tree, and only a resync
from a member's GroupInfo brings it back (tested in fogata-mls). When MLS is wired in, a
restored group in that case must resync like a device that lost its state.

**Threat model.**
- A stolen copy without the key: nothing but the header, and 256 bits to guess.
- The key without a copy: nothing. Both together are the account, as an unsealed folder is;
  the screens say to keep them apart. The key also makes copies a restore takes (it gives the
  auth key), so a thief with it could hand the person a copy of another account; the
  person sees whose it is before it replaces anything.
- Someone who changes a copy cannot make it restore anything but what it held: every frame is
  bound to its place and to the header, and a copy for another key does not open.
- Someone who knows only the public half (a backup of a sealed folder shows nothing of it,
  but one of a plain folder does) can make a copy that opens with the key, but not its proof:
  the restore refuses it ("this copy was not made by this account").
- Someone who writes in a sealed folder without its key cannot change what copies are made
  for: `recovery.bin` must open with the folder's data key, or no copy is made. In a plain
  folder (one whose person chose so, 5c) nothing protects it, as nothing protects the
  identity there.

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

## 8. Testing

- Unit tests for the crypto and wire formats.
- Fuzzing of every decoder of what comes from outside (other devices, pasted links, files on
  the disk): long runs of damaged inputs and libFuzzer targets, `tools/fuzz.sh`; see
  `docs/fuzzing.md`.
- `tools/netlab`: network namespaces with a simulated public network, a local DHT bootstrap
  node and NATs of different kinds (port-preserving, random port per destination), to run
  the connection scenarios automatically: members connecting directly, by scheduled punch
  or through a member; joining with an invite; becoming friends with a link or a code and
  meeting again later; accepting a request after the requester went offline; removing a
  friend; a file shared in a room between two hard NATs through a member; direct messages
  written while one friend's computer sleeps and the other's crashes, each arriving once and
  in order; the network check; and moving an account (5e) on one local network over mDNS
  (`transfer-lan`), between two cone NATs over the DHT and a punch (`transfer-cone-cone`), each
  followed by the moved account talking in its group, and between a cone and a symmetric NAT,
  which must say the two could not reach each other and change nothing
  (`transfer-cone-symmetric`); a release that spreads from the publisher to a friend and on
  to that friend's friend on another platform, while a stranger gets nothing
  (`update-chain`), and a modified copy whose unknown key, broken signature, older build,
  oversized package and damaged chunks are refused (`update-refusals`).
- `tools/beta`: the release tooling's tests (`test_release.py`, and `test_ci_builds.py` for the
  fetch and check of the builds GitHub makes), the macOS updater's
  (`test-macos-updater.sh`: what it refuses, with bundles signed for the test), and on Linux
  the Flatpak's, each in a flatpak installation of its own: what flatpak takes from the
  loopback origin (`test-flatpak-origin.sh`) and where the data key is kept and how it moves
  there (`test-flatpak-keychain.sh`, in a session of its own).
- `app/test`: the app's unit and widget tests (about 160 files: every screen and widget,
  the state the screens read, formatting, the order of video codecs in a description...),
  run with `flutter test` on every system.
- Compatibility between versions: the judgment of level two pinned on a corpus and compared
  with the frozen level-one copy (5.1); `crates/fogata-core/tests/previous_release.rs`, the
  command line of the last release against this version (weekly on GitHub, and before every
  release: docs/releasing-beta.md); and `crates/fogata-core/tests/old_copy.rs`, copies from
  before a given feature (run by hand where such a build is at hand). The plan for later
  levels is under "Later levels" in 5.1.
- `app/integration_test` (run on Linux, macOS and Windows): the application driven on a
  desktop with a fresh data folder:
  every screen (`screens_test.dart`), and a whole session with another device
  (`with_friend_test.dart`, with `app/tool/peer.sh` or `peer.ps1` running the CLI as the
  other device:
  friend link, group, room, chat, a file downloaded and checked, the other leaving), the
  window buttons (`window_test.dart`), and a call between copies of the app, on one
  computer or on several (`call_test.dart`, two to six copies: after the friend links are
  carried over, the copies take turns through the room's chat; they connect through their
  media bridges, hear each other, see a mute, notice a change of microphone, and, as
  asked, show a camera and a shared screen, logging round trips, losses, freezes and video
  sizes), and direct messages between two copies (`direct_test.dart`: a friend link, writing
  both ways, pictures that come by themselves and files that are fetched, the timer for
  both, clearing and asking again, what is written while the other copy is closed and goes
  once it starts again, and a chat that can only be read once the two are no longer
  friends). On Linux it runs with virtual sound devices (a PipeWire null sink, named by
  `PULSE_SINK`, which also takes the application's own sounds, and sources fed by it), a
  made-up camera (`FOGATA_TEST_CAMERA=1`, see `app/third_party/flutter_webrtc/FOGATA.md`)
  and a virtual display (Xvfb), so it touches nothing of the computer's own
  microphone, speakers or screen; a copy entered with the microphone off must not record at
  all, except from "Fogata silence" while it shares a screen's sound. It refuses to run without a data folder of its own. A virtual machine on a computer
  with a firewall may need a rule letting the machine's network reach that computer.
- Real-world tests between devices on different networks.

## 9. The first milestone (done): the CLI prototype

Kept as the record of where Fogata started; the command line of today is in docs/cli.md.
Two machines on different networks find each other using only the group secret and exchange
encrypted text messages.

```
fogata id                         Create or show this device's identity.
fogata group create <name>        Create a group (this device is first on its ladder).
fogata group invite <name>        Print a single-use invite (managers).
fogata group join                 Join through a member that is online, with the invite typed
                                  when asked (an argument shows in the list of processes).
fogata group members|remove|move-up|move-down|managers|add|rename|leave
fogata group list                 List the groups of this device.
fogata chat <group> [--nick N]    Connect to the group's members and chat on stdin/stdout.
```

The CLI prints how each member was reached (LAN, direct, hole-punched, through a member, which
it names when that member is in the same group).

Done means: the netlab scenarios pass (direct, scheduled punch, member coordinator, member
bridge), and two machines on separate networks (home fibre and mobile data) exchange
messages, with the connection type shown.
