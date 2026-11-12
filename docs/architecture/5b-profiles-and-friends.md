Part of [Fogata architecture](../architecture.md).

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
old one, but for group logs, which grow by what is new (5.1) (`fogata-core/src/writer.rs`).
At each change of a group the node hands the core a copy of its log to save: copies share
the entries, each kept once, and what was worked out of them until one changes, so a copy
costs little whatever the log's length (a copy of a 3 MB log takes well under a microsecond;
200 changes made on the node and copied took it 60 ms before, 17 to 25 ms after, the rest
being the changes themselves). A slow disk delays the files, not the events. A write
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
