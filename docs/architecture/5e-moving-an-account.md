Part of [Fogata architecture](../architecture.md).

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
