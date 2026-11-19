Part of [Fogata architecture](../architecture.md).

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
  messages, opaque to the node) over the member's best connection that is not suspect
  (best path, QUIC first; see `request-response` in 3), only between members of the room's
  group. The device with the smaller id makes the offer. Each description carries its
  network candidates, gathered before it is sent (quick: the only server is the
  device's own bridge), so one message per side brings everything and no candidate can be
  lost or reach a later connection. A signal to a member this device is not connected to
  yet is dropped, and members connect to each other at different moments, so the side that
  offers offers again while there is no answer (after 5 s, then up to every 30 s), and the
  other side asks for an offer the same way, the first time as soon as it makes the
  connection's place (a side that kept a connection from before, with a call there that is
  gone, offers only when asked). An offer still waiting for its answer, on a connection
  made less than 15 s before that has not failed, goes again as it is (the same
  connection, number and call), and the other side answers a copy with the answer it
  gave: when signals take seconds each (a computer so loaded that each waits seconds to
  be sent and handled), a new offer at each try threw away the one whose answer was on
  its way, and a pair took 25 to 50 s, up to two minutes, to connect; now one offer and
  its answer, however slow, as long as the answer comes within the 15 s. A copy of a lost
  offer does what a new one did, at the same moment. Copies before this one (0.1.0-beta.11
  and before) answer a copy anew, on a new end of theirs, and the first answer would then
  be set on a connection whose other end is closed: a participant whose signals say
  nothing of copies (see `w` below) gets a new offer at each try as before. One heard from
  not at all yet gets a copy, which costs such an older copy, when its answer takes more
  than 5 s, one more round of 15 to 20 s. When the core loses its link with a participant
  (a network change, a short outage) and has it again, a connection with it that is not up
  and whose offer or answer went while the link was down, or was still being agreed when
  it went, is tried again at once (the smaller id offers, the other asks), rather than
  after its tries ran out; one made after the link came back is not, so the two sides'
  tries do not undo each other, and a whole network coming back tries each participant
  once. A participant that leaves and enters again at once may be named in every list of
  participants the application reads meanwhile: the room's own event that it left lets its
  connection go at once, and each call has an id of its own, named in its offers, answers
  and askings, so a participant whose signals name another call than its connection was
  made with (it left and came back, or its application started again faster than WebRTC
  finds a connection dead) gets a new connection at once, and what its ended call still
  sends is dropped. The call a participant had when it left the room is kept while this
  call goes on (the last 64 to leave), with the calls of theirs taken for ended. Only an
  answer to a `which` naming another call (below) ends one for good. A call taken on a
  guess (a signal of another call taken at once, or after questions that went
  unanswered) changes the call known without ending the one before, so that a later
  signal of it is asked about rather than dropped, except for a participant not known to
  answer a `which`: the one before is then taken for ended (a late signal of it would
  otherwise end the call they have, and so back and forth), but as a guess, which any
  signal of that call saying that it answers (`w`) undoes, as it may be the call they
  have (they came back with the same call, and a late signal of an older copy came
  first); copies before this one never say so, and a late signal costs at most one
  question. An answer names a new call only when it answers this side's latest offer (it
  was made after that offer, by the call they had then). An offer or an asking that names
  another call than the one known may also be a late one of an older call this side does
  not remember (both sides made their calls anew within seconds, as this side's memory of
  who left starts empty with its new call; or more than 8 of their calls ended): taken as
  theirs, it ended the call they have, every later signal of that one was dropped, and the
  pair stayed unable to connect until one of them left the room or started again. So such
  a signal is held, and the sender is asked which call it has now (`which`, with a random
  token made for the question); its answer (`this`) carries the token back and names the
  call it has as it reads the question (`now`). A late signal cannot carry a token made
  after it was sent, so only that answer decides: the call held is taken (and its signal
  handled as it came) when the answer names it; when it names the call known, the one held
  is older and ended; another, and that is their call now, connected to anew. Every change
  of the call known drops the question, and a signal of yet another call held meanwhile
  gets a question of its own, so an answer made before that call or that change cannot
  undo it. This costs a call that is really new one question and its answer (two signals'
  time) when a call of theirs is known; nothing is asked when none is (the room showed
  them gone, or this side just started). One question at a time per participant (one held
  signal, the latest), asked every 3 s with the same token, five times: with no answer
  meanwhile the call known is kept, and an answer that comes later is still taken (a new
  call goes on offering or asking, which asks the same again); the participant is shown as
  it was meanwhile. Signals come from the device they name (the network layer takes the
  sender from the member connection that brought them), so a member can make others ask,
  or answer, only about its own calls. Copies that do not answer say nothing of it: those
  that do mark their offers, answers and askings (`w`, which also says they answer a copy
  of an offer with the answer it had), and a participant whose signals never did
  (0.1.0-beta.11 and before) has a signal of another call taken at once as before, with
  the same weakness, until it is updated. A participant that came back with such an older
  copy, the room showing nothing, is asked all the same and never answers: once the five
  questions are gone unanswered, a held signal that says nothing of answering is taken as
  its copy's signals are, about 15 s later than before, and it is asked no more until a
  signal of theirs says again that it answers. A late signal of an older copy whose device
  has been updated since loses to the answer of the copy it runs; when every answer is
  lost, the older call is taken after 15 s, but their call of now is not put among those
  that ended, and its next signal is asked about and taken. As an offer goes again as a
  copy, two calls of theirs may each answer it: an answer of another call to this side's
  latest offer is taken at once only while its connection has no answer yet; once it has
  one, the participant is asked as for an offer, and when that other call is theirs now
  the connection, which holds the answer of their ended call, is made anew at once. A
  participant who goes out and back with the same call and whose first signal after it
  came back is a late one of an older call has that one taken, and the call they have
  asked about at its next signal, whether or not it was known to answer. Still not
  covered: such a participant on a copy before this one (the call they have is taken for
  ended, until they or this device make a new call); and a `which` to a device whose room
  has not named this one yet takes one of the four places its signals wait in, until it
  does. Each signal goes on its own, with no order between two, and may come twice: a copy
  of the first offer a connection was made
  for (the same call and number), or of a screen's sound offer already answered, gets the
  same answer again and leaves the connection as it is (a first offer's copy is answered
  anew when that connection failed or closed). The numbers of the offers a call makes for
  a participant only rise, from one connection to the next and when the participant is
  back in the room (from the clock in milliseconds, or one above the last made for them,
  remembered for the last 64 to leave), so an offer of the same call with a lower number
  than the last that made a connection here is a late one and dropped: answered, it made
  anew the connection that works. Only from copies that mark their signals (`w`); older
  copies number from anywhere. A restart makes a new call, whose numbers are not compared
  with the one before, so a clock set back does not hold its offers back. Only the room's
  participants get
  a connection: a signal from a member that is not in the room is dropped, and one that
  arrives before the room says its sender is in it, or before this device's call has made
  its connection with the sender, waits for that (the last 4 of each sender): those
  already in a room offer as soon as they hear a device came in, a moment before its call
  has its media bridge and microphone, or is made at all. The node already drops signals
  that reach a device in none of the group's voice rooms, those over 64 KiB (a description
  takes a few kilobytes) and, from each member, those past a burst of 32 and then 8 a
  second; from a member not in this device's room (its join may still be on its way),
  those past 4 and then one every 5 seconds, so they never reach the application in
  numbers. Such a signal also has the room asked of its sender (once every 15 s at most),
  so a join that was lost does not wait for the sender's next presence.
  The application drops a description over 64 KiB or with more than 4 media sections, and an
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
  A `state` message tells the others what is on, for the muted icon and the tiles. It
  carries a number that only grows (`s`: the sender's clock in milliseconds, or one more
  than its last state's, from 0 below 2^53), and one below the newest taken from that
  participant arrived late and is dropped. It also names the sender's call (`sc`, the id
  its offers, answers and askings carry as `c`): one naming another call than those
  signals named is dropped (a late one of a call that ended, or one of the next call ahead
  of its offer, whose state comes again once its connection is up), and numbers are
  compared only within one call. What is kept starts over when the participant comes into
  the room again and when its offers, answers or askings name a new call of theirs. One
  without these fields (older copies) is taken as it comes.
- **The microphone is open only while it is on**, and its track is closed while muted.
  WebRTC records whenever a send stream of sound runs, with a track or not: a voice
  channel's whose encoding is on, or the screen's sound channel's. That one sends only
  while the screen's sound goes out in the call's own channels: on Linux with the
  microphone off too, recording then from the silent input; on Windows only with the
  microphone on; on macOS never (its sound goes on connections of its own).
- **A mute** first silences every sender, as it is asked, whatever each connection is
  busy with: on Linux and Windows it disables the microphone's track (WebRTC mutes a
  sender's stream when its track is disabled); on macOS it takes the track off every
  sender, as the call always did there, since disabling it would have WebRTC-SDK's audio
  module mute Apple's voice processing input instead (flutter_webrtc's FOGATA.md, "A mute
  silences every sender at once"). The state message and every participant's step
  go out at once too, without waiting for a microphone still opening, which lets go of
  what it opens once it finds the microphone off. The microphone is then closed or
  opened, changes one after the other; an unmute answers its caller once the microphone
  is open.
- **Each participant's two channels of sound** are brought to what the microphone and
  the share ask by one step, in that participant's turn with its signals and apart from
  the others, reading what is there each time. It runs whenever anything it reads
  changes: the microphone, its stream (opened or closed, late at a call's start
  included), the share, its sound, its capture, the silent input, the connection coming
  up, the screen the participant watches, their voice taken or not, and every description
  agreed (offered or answered). Each of these is changed only through a setter that runs
  the step again, and a test reads the sources for any other assignment.
- **With the microphone off**, a channel of sound with nothing to send that is agreed to
  send on a connection that has been up has the encoding of its send stream stopped
  (`active: false`, `RTCRtpSender.setParameters`, read back after: macOS's plugin answers
  yes whatever WebRTC did), whatever the connection's state now: WebRTC stops the
  stream, and once none runs it stops recording, the same stop as for a channel that no
  longer sends. A voice channel so stopped keeps its direction, and unmuting turns its
  encoding on again: the voice goes out at once, with no signal to wait for. Any other
  voice channel (a connection never up yet, one made while muted, one the other side no
  longer takes the voice on, or one whose encoding would not stop) only receives while
  muted, its encoding left as it is: WebRTC prepares the microphone whenever a channel
  comes to send (`WebRtcVoiceSendChannel::SetSend` calls `InitRecording`), which on macOS
  opens it, and lets go of that only once a stream starts and stops. Its next unmute
  agrees the channels again on the same connection (only the device that made the first
  offer offers again, the other asks it with an `update` message, so two offers never
  cross).
- **With the microphone on**, the step turns a voice encoding on only once the
  microphone's track is there to send; a send stream WebRTC makes while the microphone
  still opens starts with its encoding on, as WebRTC makes every stream, and goes without
  a track until it is there. A channel's direction is set, and agreed again, only where it
  differs from the one this device last asked for on that connection, which the call
  keeps with the connection (the plugin tells a channel's direction only on macOS); every
  direction is set in one place, which keeps it. An encoding that is not taken, the
  voice's or the screen sound's, is tried again 1, 3 and 7 s later. The voice's still
  not taken, the connection is made anew: where this device offers, it offers a new one
  at once; where the other side offers, this side closes its end and asks for a new offer
  (`again`) every few seconds for 30 s. The other side takes that only once its own end
  is no longer up and had both descriptions more than 15 s before; how soon WebRTC finds its end no
  longer up is WebRTC's own, and at the latest that end fails after 30 s with nothing
  received, after which the other side offers anew within 2 to 6 s by itself: about 45 s
  at worst in all, with neither voice nor pictures between the two meanwhile. That is
  done three times at most for a participant in a call; after that the connection is
  left as it is, with a line in the call's log, and nobody is told: in a call of two the
  microphone check may then say the microphone reads nothing, which is not what
  happened.
- **The microphone check** judges a connection once its voice channel follows the last
  change of the microphone with its encoding on and the live track, or 10 s after the
  change all the same, so that a connection whose voice cannot start is told rather than
  nothing. When the shared screen is captured anew, the new capture's sound reaches
  every sender before the old capture is let go of. Muting while the screen's sound goes
  out on Linux, that sound is neither stopped nor agreed again while the silent input is
  chosen. Older copies, which still mute by agreeing again, work with this in both
  directions: their offers are answered as before, and a channel kept sending answers as
  one that sends.
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
  so text stays readable, until that leaves it choppy. Each person who watches gets a
  connection of their own (those sent the screen at one size share one encoder of it, see
  below), and the sharer reads each connection's statistics every 2 s: under
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
  **The screen's upload is shared among its watchers.** Each watcher gets the stream on a
  connection of their own, so the sharer sends the watchers times one stream. Measured on
  GitHub's runners (90 s each): a moving screen (a film, or text scrolling in a window)
  sent about 6 Mbit/s to each watcher at 540, 720 and 1080 lines alike (5.4 to 6.1), as
  the limit above and not the size decides there, and about 11 at 1440; three watchers at
  1080 lines took 17 Mbit/s in all (19 at most), five at 720 lines 24 to 28, more than a
  common home upload of 5 to 20. A screen that hardly moves sent about 1 Mbit/s to each at
  1080 lines, 0.4 at 720 and 0.2 to 0.36 at 540. One or two watchers get what the screen's
  size and rate allow, as before: two at the usual quality already take 12 Mbit/s between
  them, and the size's ceiling treats them alike too. From three on, the same 12 Mbit/s is
  divided among them, each getting the smaller of the above and its share, never under
  1.5 (as much as the camera's most): three get 4 each, four 3, five 2.4, six 2, seven
  (the most a call of eight has) about 1.7. That fits a common upload with room for the
  voice and the camera. A share of 6 for two would have left a 1080p screen at 60 frames a
  second (12.4) or a 1440-line one (11) at under half of what one gets, dropping frames as
  soon as it moves, and each watcher's connection would then have stepped it down on its
  own, splitting them among encoders. From three on every watcher gets the same limit,
  whatever size their connection lets the screen go out at (the limit above is never under
  6 Mbit/s), so those sent one size keep sharing its encoder (below), which takes the
  lowest bitrate of them. Only watchers whose connection is up count: one kept out, or
  making its connection again after it failed (5 to 30 s), still watches (and still counts
  for the size's ceiling, which would otherwise change everyone's size twice for one who
  comes back), but takes nothing of the budget meanwhile. The shares follow the number at
  once, as the camera's steps follow the number in the call: when someone starts or stops
  watching, or a watcher's connection comes up or goes, the others' connections are told
  their new limit, each queued on its own turn, and the one who came is told as theirs is
  set; a watcher who leaves the call without saying so, or one whose connection closes
  without telling, is counted out at the next reading (2 s). After a change the shares wait
  a second before following another, and then follow the number there is, telling every
  watcher, so that someone turning their watching on and off again and again does not have
  every other connection told each time. The application cannot know a person's upload,
  and this does not guess it: a slower one is left to each connection's bandwidth estimate
  and the screen's steps. Measured in the call lab with an upload limited as a home
  connection's is (`LAB_UPLOAD` in `tools/call-lab/measure.sh`: only what the sharer sends
  goes through the rate, behind a 100 ms queue), a moving 1080p screen degrades together for
  everyone rather than freezing anyone: three watchers stay at 1080 lines and 25 to 30
  frames a second at 12 Mbit/s and above (20 where the computer's processor held the encoder
  back) and 15 to 20 at 8; five stay at 720 or 540 lines and 15 to 30 frames a second down
  to 8 Mbit/s, the limit applied from the start or after a minute alike, and with 1 % loss
  and 40 ms round trips at 8. At 4 Mbit/s, a third of what the streams ask for, a watcher
  dips under 5 frames a second for a few seconds now and then as the size is tried again;
  with five, in three runs of four no watcher stayed under 5 frames a second for more than
  14 s at a time, and in one three of them spent 33 to 61 s of the first 80 under it (35 s
  at once at most), as the connections' estimates of the one upload start uneven and the
  smaller size's shared encoder follows its lowest. Each connection's voice and control take
  about 170 kbit/s whatever the rate, and the QUIC and TURN framing an eighth to a quarter
  on top of what WebRTC sends. An earlier trial that limited the whole loopback instead,
  where every packet between two copies passes three times and every copy's voice and
  feedback too, left two of five watchers at 2 frames a second for three minutes at "12
  Mbit/s": that was the limit counting the traffic three times over, not the call.
  The processor lowers the screen for everyone, a level at a time: the rate halves first
  (60, 30, 15 frames a second, a level that sends the same as the one above being passed
  over), then 720 lines at 15, then 540 at 15, 10 and 5; it goes down when WebRTC says an
  encoder is held back by the processor (`cpu`) and sends well under its level's rate, or
  when the encoders together keep 1.5 cores busy, and back up a level once the one above
  would fit. A single step from 60 straight to 15 once left a screen there: to come back,
  the encoders had to fit in a quarter of what they took. With an encoder for each watcher
  (the shared encode below off), a share starts lower the more watch, and goes there at
  once when someone starts watching (not at the next reading): the full screen for one or
  two, 15 frames a second for three, 720 lines at 15 for four, 540 lines at 15 from five.
  In the lab (a 1600x900 screen, a loaded computer), three watchers kept 15 frames a
  second at full size, while five started at 720 lines fell to 10 and seven to under 10.
  With the shared encode a share starts at the full screen whatever the number, and the
  processor's load counts the one encode of each size once, however many connections
  report its time.
  The number watching also sets a ceiling on the size, on top of the quality chosen (never
  more than chosen, nor than the screen is): one or two watchers get the quality chosen,
  three or four at most 1080 lines, five or more at most 720 (each in a 16:9 box, as the
  qualities are). It is there for the upload: a screen that hardly moves, as documents,
  code and slides mostly do, sends to each watcher what its size calls for (the numbers
  above), and a moving one above 1080 lines, which one or two watchers may have, takes 11
  to 20 Mbit/s to each. It was first there for the processor: before the shared encode,
  the mesh encoded the screen once per watcher, and each encoder's memory and work grew
  with its pixels (shared to seven, a 1600x900 screen took the sharer about 1 GB of memory
  and a core, `VideoEncoderQueue` threads and one allocator arena of 27-40 MB each), and a
  screen shared at its own size (1440 lines, 4K) would take two to four times as much per
  encoder. Under the ceiling a level that sends the same as
  the one below it is passed over, so when the processor allows more, a big room gets
  frames back rather than size. The ceiling comes down as soon as more watch, before the
  new watcher's encoder starts, and goes back up only once fewer have watched for 20 s (to
  what the most that watched meanwhile allow), so someone coming and going does not make
  every encoder change size each time. Like the levels it changes only what each connection is told
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
- **A shared screen is encoded once for all who watch it at one size.** WebRTC gives every
  connection that sends a track an encoder of its own; Fogata's libwebrtc (wrapper patch
  17 on Linux and Windows, Apple patch 7 on macOS, `tools/webrtc`) has the connections
  that send one screen at the same settings share one: the screen's source tags its
  frames, each frame is encoded once when any of them asks, and every connection gets the
  image with its own RTP timestamps. Each connection still packetizes, encrypts, paces and
  estimates its bandwidth on its own: **nothing on the wire changes**, and watchers of any
  version see no difference; an older sharer just keeps an encoder per watcher. A watcher
  joining gets a key frame at once (made for all, or one just made), as its own encoder
  made one; key frames a watcher's receiver asks for are given to all, at most one a
  second, and one owed while the screen stays still is made of its last picture again;
  the bitrate is the lowest watcher's. So one slow or lossy watcher costs the
  others for a few seconds (a blurrier picture, at most a key frame a second) until it is
  isolated: a watcher under half the others' bitrate for 3 s, or one asking for a key
  frame three times in 10 s, gets an encoder of its own, and the screen's steps (above)
  then judge it as before; it rejoins once its bitrate has been the group's for 10 s
  (twice as long, up to 160 s, each time that did not last 20 s). A watcher whose
  connection pauses (no bitrate, network down) gets the frames of the next 250 ms and then
  nothing; when it asks again it takes up where it was if every picture it missed is still
  kept (eight), and waits for a key frame otherwise. A connection that stops sending costs
  the others no key frame. An encoder that fails three times in a row, or cannot start,
  gives every watcher an encoder of its own for a minute; one frame that fails is a frame
  dropped for all. Cameras, a disabled track's black frames and hardware encoders are
  never shared, and a camera's connection has its encoder as without it. It is a field
  trial the plugin sets (`Fogata-SharedScreenEncode`), which
  `FOGATA_SHARED_SCREEN_ENCODE=off` leaves out, to measure before and after on one build
  (`tools/call-lab`); each connection's statistics then name its encoder
  `libvpx (shared)`. On macOS too, where the plugin then gives libvpx's own encoder and no
  simulcast adapter over it (the adapter does not say it encodes in software before it is
  initialized, and nothing was shared through it): a Mac sharing its screen to two watchers
  took 60 % of one core instead of 87 % (`tools/webrtc/MACOS.md`, "Checking patch 7").
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
  pause of 4 to 6 s while both are in the room. The 15 s count from the moment it has both
  descriptions when that is later (the offering side's answer came slowly), so a
  negotiation that goes on is not thrown away: 30 s at most in all, as the answer must
  come within the first 15 s, and nothing the other side sends lengthens it. An asking
  for a new offer (`again`) is let pass by the same measure. Answers slower than that
  (signals of more than about 7 s each way) still have their offer made anew. Each offer carries a number that its
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
  goes, so it never becomes another microphone or the default one. Choosing the
  microphone WebRTC records from already (the call chooses it as it starts, and the
  microphone opens it again when it is turned on, maybe once someone already hears this
  device) leaves the recording alone: WebRTC would stop it and start it again at once,
  and PipeWire 1.0 answers that, for a remapped or filtered microphone, with silence until
  nothing records from it. It is the same microphone while it has the same id at the same
  place as the one last asked for, with nothing counted since: on Linux a source added to
  the sound server or removed from it, or the server lost and found again; on Windows a
  change of state of any endpoint, outputs too; and the voice engine's start. Otherwise it
  is chosen anew. Anything counted between the call's start and the microphone's opening
  (a Bluetooth headset connecting, the monitor of a new output, the application's own
  silent input for a shared screen's sound) has the recording started again once, as
  before, so the silence is avoided only while nothing changed in between. With WebRTC's
  ALSA module (Linux without a sound server) nothing is counted, so a device unplugged and
  plugged in again at the same place is taken for the same and opens again only when the
  microphone is turned off and on; the system's default input changing while the default
  is chosen is not counted, as the sound server moves the recording itself. On Linux a sound server
  that goes away while the application runs (PipeWire or PulseAudio restarted, as after an
  update) is connected to again by Fogata's libwebrtc itself (0.5 s after, then after 1, 2
  and 4 s, then every 5 s, giving up on a server that does not answer in 5 s), which then
  takes the chosen speaker and microphone again by name where they are still there and
  plays and records again as before; meanwhile it lists no device at all, the call says
  nothing of its microphone and does not turn it off for reading nothing, and a server
  gone over 5 s is told, and its return too. It used to keep a dead connection until the
  application restarted, listing only "System default".
  On macOS one audio engine plays and records; our copy of flutter_webrtc remakes it when
  it stops while it should play or record (its FOGATA.md, "The others' voices play without
  a microphone"): as it was first, as an engine also stops when the system switches
  devices, and without the microphone only if that does not run, trying the microphone
  again by itself after 2, 10 and 30 s and at each change of the devices or of those
  chosen. The engine's input and output are one audio unit with one device (Apple's voice
  processing is off: WebRTC cancels the echo), so the speaker and microphone chosen are
  given to it by our copy, not by WebRTC's audio module, which lost its engine, and the
  microphone with it, for any output chosen other than the microphone's own device: one
  device when both are on it, else a private aggregate of the two (its FOGATA.md, "The
  devices chosen, with the microphone"). An output that cannot be put with the
  microphone (another sample rate, or a pair the engine refused in the call) is not given
  to the engine at all: it follows the system's devices, muted or not, as with nothing
  chosen; a recording lost when the engine refused at an unmute is started again by the
  call, stopping and starting its voice encodings. The call reads what the engine uses every 2 s,
  tells when it is not the output or the microphone chosen (each time that begins), and
  shows the one in use beside the person's choice, which stays as made. While the
  microphone is left out the call still shows it on, and the call's own check (8 s with
  nothing read or sent) says it could not be used and turns it off.
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
  left out, so all three are named); on Linux and Windows those settings reach WebRTC's
  audio processing only since Fogata's libwebrtc applies a microphone source's options to
  it (before, it kept its defaults: WebRTC's suppressor on at its highest level). Both
  suppressors at once were measured too: 8 dB less steady noise than RNNoise alone, but
  the clean voice 1 dB quieter and 2 dB thinner around 500 Hz (RNNoise alone: 0.2 dB),
  and "off" would no longer be off. Measured on
  Linux with real recorded speech and made-up noise, 20 s off and on in turn (level
  between words; voice level while speaking): steady fan, hiss and hum about 30 dB under
  the voice: -47 to -62 dBFS (WebRTC's own: -46 to -62), voice 1 dB lower; the same noise
  12 dB louder: -32 to -62 dBFS (WebRTC's own: to -49); noise alone: -41 to -73 dBFS
  (WebRTC's own: to -58), often silence that Opus then does not send; typing: 5.6 dB lower
  (WebRTC's own: 1.4 dB); clean speech: 0.2 dB lower, the octave around 8 kHz 3 dB lower.
  RNNoise takes about 60 µs of every 10 ms at 48 kHz, 0.6 % of one core here, and the same
  on WebRTC's audio thread in a real call (WebRTC's own suppressor: 0.1 %). A steady noise
  that comes back at once after being held down is taken for speech for a moment: WebRTC's
  echo canceller, made anew at each start of the recording, holds the microphone about
  23 dB down while the other side's sound plays in its first seconds and then lets go at
  once; in the call lab RNNoise let the noise through for 0.2 to 0.5 s after that as a
  rule, once for 4.5 s. The lab's noise check (`tools/call-lab/mic.sh noise`) therefore
  plays nothing at the copy it measures.
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
  noise suppression settings, which no longer matter: it never goes through the audio
  processing, and Fogata's libwebrtc never lets a custom source's settings reach it
  (WebRTC M156 does not set the processing from a channel's source, as was thought).
  Fogata's libwebrtc (Linux, Windows) also keeps WebRTC's processing of the microphone
  the same whatever the screen's sound does: WebRTC took its output for
  unused, and passed the microphone through with no echo cancelling, noise suppression
  or gain control, whenever the last channel touched had no sender with a track (a
  connection closed as another took its place), until a screen's sound started going
  out; it now counts every channel's senders. And it processed the microphone in as many
  channels as the channel sending the most, so a screen's sound in stereo initialized
  the processing again as it started and stopped; the microphone is now processed in
  one channel always. Its echo canceller starts anew each time recording starts (a
  call, or the microphone on again with nothing else going out): kept from the last
  call, it held that call's alignment of the played sound with the recorded one and let
  the other side's first second or two of speech go back to them (every system, macOS
  too). macOS's WebRTC
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
