Part of [Fogata architecture](../architecture.md).

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
