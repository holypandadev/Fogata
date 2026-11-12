Part of [Fogata architecture](../architecture.md).

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
  `direct_chat(friend)` (its newest items with each attachment's state, the messages
  pinned, whether the chat is connected, what it takes here, and a `version`),
  `direct_storage()`. A chat on screen is then read only in parts: `direct_chat_older`
  gives the items before one the screen holds, by that item's place in the chat (its
  `seq` and id, which never change, so the page before it is the same however many came,
  changed or went after it), down to a given place when asked (a pinned message brought
  into view, in one read); `direct_chat_changes` gives what changed since a `version`
  among the items from the oldest held on: those that came or changed (delivered, a file
  coming, fetched or removed, played, pinned or unpinned), the ids of those that went or
  no longer show, and the pinned messages only when they may have changed. Each item
  carries, in memory only, the stamp of its last change, from one count for every chat,
  and a chat remembers its last 1024 removals: a view older than the chat (read again at
  a start) or than the removals it remembers gets the items from the oldest it held on
  whole instead. The screen merges what comes into what it holds, reads the page one
  read at a time in the order asked, and holds only the chat shown; the others are the
  list's summaries.

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
