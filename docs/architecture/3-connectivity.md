Part of [Fogata architecture](../architecture.md).

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
| kad | Amino DHT (`/ipfs/kad/1.0.0`), always in client mode: Fogata devices never serve the DHT. The routing table is bootstrapped at the start, on waking up, every 30 minutes while it holds fewer than 20 peers and every 2 hours once it holds more (the library's own refresh is off; it still bootstraps when a peer joins a thin table). Each bootstrap is a minute of lookups through dozens of strangers. A lookup goes on through the DHT nodes an answer names, never through the providers it names: a node on the way can name any number of made-up ones at an address that never answers, and dialling them as DHT nodes held the lookup's few places until each dial timed out, while the nodes that keep the real announcement waited (vendor/libp2p-kad/FOGATA.md). |
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
  The dials of one lookup's candidates, 64 at once at most, must not stop the device's one
  UDP socket. On Linux a packet to an address on the device's own network that no host
  answers for waits in the kernel while the neighbour is looked for, and counts against
  the socket's send buffer until it is dropped; some 90 dials to one such address filled
  the default buffer, and the socket sent nothing to anyone for seconds. The socket asks
  for a larger buffer there (`vendor/libp2p-quic`, change 4). In `dht-liars` the made-up
  members' address is on the devices' network and answers even ARP only after 12 s: with
  the default buffer the requests to the real DHT nodes went unsent for 8 s and timed out.
  A limit: the kernel gives at most what `net.core.wmem_max` allows, twice the default
  buffer where that is left at its default, and each such address holds up to one default
  buffer of packets, so made-up members spread over three or more addresses of the
  device's own network that no host answers for could still fill it for a few seconds.
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
  device's memory only: each member answers with what it keeps, and two members that keep
  different amounts still list the same people and files, alike for a member that asks
  either, and lose nothing for asking each other (`node::room` test
  `devices_keeping_less_chat_still_agree_on_the_room`); only how far back the chat goes
  differs. The shares stay bounded by group, the same on every member: a device cannot
  let go of shares the others list, or it would answer a member that asks with another
  room. What a device keeps of the shares is the same set, held once: each event as the
  message sent, which the voice room's latest events (named in presences, given to a
  member that asks for them by id) share rather than copy, and a video's first picture in
  that message only. The rooms list a video with an empty picture
  (`RoomTracker::keeping_pictures`); the screens get the room they show with every picture
  read again from its share's message, opened with the key of its epoch (some 9 ms for 47
  of the largest pictures, a person's whole part of a one-room group, in a release build on
  a Mac; at that rate about 75 ms for a whole room's 24 MiB), and the pictures of that room alone stay
  read, until another is shown (`ShownPictures`). A file of the room shared on from here
  goes with that picture. Measured (`measure_device_room_memory_*`, ten groups each with
  eight devices of a flooding member in all ten rooms, macOS release build): 633 MiB of
  chat and 200 MiB of shares as counted, 1.28 GB resident, with each group's bounds alone;
  127 MiB of chat and the same shares, 0.78 GB resident, within the bound of all the chat
  with the pictures kept read and the latest events copied as before; 0.55 GB resident
  with both kept once. What is left grows with the groups by their shares as sent, bounded
  per group, the same on every member.
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
- **Summaries, and what is held in memory.** Beside each chat's journal is its `summary`
  (sealed whole for the chat in a folder with a key, plain JSON without one): what the list
  of chats shows (the last line, unread messages, the timer, whether it can only be read),
  what its attachments take, when something in it goes next, how many of this device's
  items wait for a receipt, its oldest charge, the files kept here (which its group serves)
  and up to 64 of the friend's files not here. It is worked out whole from the chat each
  time records are written (`model::Summary::of`), and written after them, with each
  file's length and last 16 bytes (a sealed record's tag). A start takes a chat from its
  summary only while the journal and pins end exactly so and, sealed, are chains that end
  as their tails say, with no rewrite cut short; otherwise (a folder from before
  summaries, a journal changed, cut short or put back, a summary lost, damaged, older,
  newer or another chat's) the chat is read whole as before and its summary written
  again. An account move leaves summaries out and sealing a plain folder removes plain
  ones; both are made again. So a start reads no journal, and a chat's items are read
  whole when something changes in it: a message that comes or goes, a receipt, a file. A
  chat on screen that is not held is read back by pages instead (see below). The four
  chats used last stay in memory; the others are let go of (their
  summary kept) at the next tick or view, unless records of them are still being written
  or could not be, so one read again is the chat as written. What runs over every chat
  reads none: the list, what chats take, what waits, what runs out, old charges, what
  comes by itself; each reads a chat only when its summary says something is there, and
  then decides on the chat read whole. Room to free is checked from the summaries, and
  chosen, when needed, from every chat read whole. A chat that cannot be read again is
  written to no more until a later start. Measured on a Mac (release build) with 20 sealed
  chats of 10,000 messages: reading every chat whole took 374 ms and held some 47 MB; a
  start from the summaries takes 2.4 ms and holds nothing that shows; a folder's first
  start, which reads every chat and keeps its summary, 554 to 603 ms; a chat read when
  opened, 19 ms.
- **Reading back.** A chat's journal can be read from its end without holding the chat
  (`DirectStore::scan_back`, and `walk_back` for the items before a place with the pins
  in their places): the place of each record comes from the lengths before them, and each
  record, read from the last back, is opened as a link of its chain with the tag that
  ends the record before it. The last one must end as the tails say, so the journal's
  length and end are what this device wrote, nothing cut off its end and no older copy
  put back. A record is opened only when the reading reaches it, so a read vouches for
  the records it opened, in their order, and not for those further back: a record
  damaged there with its length intact is found when the chat is read whole, which keeps
  what came before the damage, says the chat was altered, puts a copy aside and writes
  the journal again, as at any read whole. Nothing more is lost by it than before. What
  later records say of an item (delivered, a file's state, played, not a voice message or
  picture, gone) is met before the item and applied to it. An item whose record is there
  twice (a journal written here never holds one so) is read back once, as its newest
  copy, where the chat read whole keeps its first. The pins are read whole (`read_pins`: a
  small file, the last change of each message), checked against their tails as the
  journal is. A journal or pins that do not read so where they are read are not read
  back: the chat is read whole instead. Nothing is added to the files for this, and no
  index is kept. 10,000 messages read back whole take 29 ms, the newest 200 under 1 ms
  (same Mac).
- **The chat on screen, by pages.** A chat that is not held is shown from its journal read
  back from the end, with its pins in their places (`Chats::paged_view`): its newest page,
  each older page as the screen scrolls back (before the oldest it holds, and down to a
  message asked for, as from the chat held), or the items from a view's oldest on. Its
  timer and what it takes come from its summary. Each pinned message is looked for back
  from the end until found, so a pin of an old message reads back to it, and a pin of a
  message no longer there reads back to the journal's start. A chat not held changes only
  once read, so a view read since it was let go of that asks what changed (the screen
  asks at every event of any chat) gets nothing and reads nothing: whether items are
  before the view's oldest is kept with the chat from the page that showed it, until the
  chat is read or let go of again. A view from before gets the items from its oldest on,
  whole, as from a chat read again. A file's download, which shows with its messages
  without reading the chat, counts as a change. Marking a chat read whose summary has
  nothing unread writes nothing (only the friend's messages after the mark count as
  unread, and what comes next goes after everything there), and a file kept here, to
  show, play or save, is found back from the end once the summary's list of files kept
  here says it is there. What its views open (pages, pins, files) is counted from when
  the chat was let go of: past 800 records, about four of the screen's pages of 200, the
  view that would open more reads the chat whole and holds it, as every view did before,
  and it then counts among the four chats used last. So scrolling far back, a view kept
  far back, a pin of a message gone or files looked for costs at most 800 records more
  than reading the chat whole, each time the chat is let go of; a chat looked at briefly
  is never read whole nor held, and does not push another out of memory. A chat in memory
  is shown from memory, as before. The chat is also read whole when a journal or the pins
  do not read back as last written where they are read, or are from before items kept
  their time in the chat (only a read from the start works that out), and for every
  change of it: a message that comes or is sent, a receipt, a deletion, a pin, the timer,
  a file that comes or goes. Keeping each item once needs every id it holds and those of
  the friend's items deleted here, and what is written follows the chat held. Each page is
  read with the chats held, as a chat read whole is: a chat not held has no record being
  written.
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
