Part of [Fogata architecture](../architecture.md).

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
on the new computer.

A group's log file (`logs/<group>.log`) is its entries one after another, each after its
length, signed and not sealed (what is secret in them is sealed to the identity), read in any
order. Written whole, they go in the order `GroupLog::entries_to_store` gives (the common order,
then a raise past this version's levels); at a change, the entries it does not hold yet are
added at its end in that order among themselves (`fogata-core/src/store/appended.rs`), so a
change costs what it adds, not the whole log. Measured on a Mac (release build, a log of 4 300
entries, 3 MB): one change adds its entry in about 8 ms where the file written whole took 22
ms; what is left is mostly flushing to the disk, as is writing what was judged of the log
(about 10 ms either way). A burst of 200 changes, written in turn by the writer below, cost
the writer 109 to 122 ms in 5 to 7 whole writes before, and 36 to 78 ms in 3 or 4 saves that
add 34 KB in all after (`measure_a_burst_of_changes_to_a_large_log` in `store/judged.rs`). It is added to only when the store knows
what the file holds (it read it whole or wrote it, and finds it as it left it: its length,
when it changed and, on Unix, which file it is) and the log still holds every entry in it;
otherwise (a file changed by something else, entries the log let go of: spare ones trimmed,
waiting ones past their room, a raise replaced by a higher one) it is written whole, so the
file never holds what the log does not and what was judged of it says the same of both. It
is written whole again, in the common order, once what was added since it was last written
whole is larger than it was then: each byte is written twice on average. Not at quitting: a
file added to reads as well. What is added is flushed to the disk before what was judged is
written; a write that fails is cut off again. A computer that stops while entries are added
leaves part of one at the end: reading keeps the entries before it (that part is no damage
to set aside: it was never all there) and writes the file whole at the next change, so
nothing lands behind it. What was judged is bound to the very entries read, so entries that
reached the disk without the verdicts on them are judged again from the file, once. Entries
added together and cut short between them read as the log with those before the cut, as
after a sync that brought only part of them. Versions before read such a file as any other.
The banner and picture records beside the log are written only when they change.

A group a friend offers again, which the node judged away from its loop,
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
lines at a time (`GroupLog::activity`, `Core::group_activity`). Each line is told from the group
before it, so the lines are worked out once for each change of the log, kept with it and shared
with its copies (`GroupLog::shared_activity`), and a page is read from their end: only its
lines are copied and made into views. It is built from the log, so
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
