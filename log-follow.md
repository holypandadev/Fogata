Branch: lab/log-follow at 5751119a728fe5a393efce10644b199c70265e31, on top of lab/log-ceiling (49c97a6), which is on main 51913bfa. No merges.

Before: a level-five log can be moved to a fresh log with `Moved`, but nothing follows the move. A member that sees `Moved` keeps the old log, never learns the new one, and the app never shows the group again under its new log. That is why `RAISED_TO` stays at four.

After: a member that sees a valid `Moved` asks the other members for the new log, checks that it is the same group, runs it, and the app lists the group under its new log with the old name and history. The old log is kept 30 days so members that were offline can still follow. A move into or out of a group still waiting for the person's yes is held until they take it. `RAISED_TO` still stays at four (see "Not in this change").

## How

- **`MovedLog`** (`fogata-proto`, `node/moves.rs`): a member request that hands the new log to a follower in pages, asked only of level-five peers. A member that answers with nothing is let go; asking every member in vain waits longer each round.
- **Following in the node** (`node/moves.rs`): judging the new log's start runs off the node loop. The node checks that the log starts at `setup` and gives the old group at `Moved`'s parents (`follows`), then runs it and emits `GroupMoved` once per run. A new log already run here another way is judged and followed too, and one that is not the group is refused for good.
- **Holding a move** (`NodeHandle::hold_move`): core holds a move into or out of an invitation. If the new log was taken meanwhile it keeps running, and the move is told again only when the old group is taken here as well.
- **Following in core** (`node_events.rs`, `store/groups.rs`): `record_move` lists the new log with the old group's name and history, and marks the old group moved. It writes nothing when it refuses: a move from an invitation, into a log waiting for a yes, or into a log another group already moved to.
- **Changes written apart from a move** (`log/moved.rs`): the first member writes again in the new log the removals, leaves and adds written apart from the move, once.
- **At start** (`lib.rs`, `groups.rs`): an old group stopped beside a new log that waits for a yes is run again, together with the whole line of moves before it, once the person accepts. It also runs when a leave of it may still be undelivered.
- An offer of a log already run here is not taken as an invitation.

## Proof

Each test below fails, or does not compile, without the change it covers.

- fogata-net, `node/moves.rs` (15): a member follows the group to its new log; a start that lacks its last entry is asked for elsewhere; a move to another group is refused for good; the first member writes again what was written apart; a device let in apart from the move follows once added again; a member that answers with nothing is let go; asking every member in vain waits longer each round; judging that failed lets the member ask again; a new log run another way is judged and followed; a log run another way that is not the group is refused; a new log left here is not followed again; a new log run here of no use yet is judged once per change; a new log that waits for the person is followed once taken; a move held for an invitation is followed once taken; a hold after the yes holds nothing.
- fogata-net golden wire file: `MovedLog` request and answer, first and later pages.
- fogata-state: what was written apart from a move is written again once.
- fogata-core store (5): a group that moved gives its name to the new log; a new log that cannot be read is not written over; a move to an invitation is not noted; a move only marked is noted whole once followed; a move from an invitation or into a taken log is not noted, and writes nothing.
- fogata-core events (3): a move followed into an invitation is held; an offer of a log run here is not an invitation; a move followed on from an invitation is held.
- fogata-core `tests/moved.rs` (7): a new log kept before a stop is listed at the next start; a new log left, offered again, declined or unreadable leaves the old group stopped; a line of moves stops back to its first log; a new log taken is followed in the same run.

Local runs on this commit:

- `cargo fmt --check` and `cargo clippy -D warnings` on fogata-state, fogata-net and fogata-core (all targets): clean.
- fogata-state: 231 passed. fogata-net: 356 passed. fogata-core: 331 passed, and every integration test file but two passes.
- The only failures are four tests that fail the same way on main on the test machine. Three need a folder to be read-only, which root ignores (`files::tests::what_an_earlier_run_left_and_cannot_go_is_left`, `tests/readmitted.rs`, `tests/sealed.rs`), and `transfer::tests::a_move_that_cannot_be_committed_stays_staged` for the same reason. All of these run in CI.

CI on this commit: CI (Linux, macOS, Windows and the Linux checks), call checks, the network lab and the Flatpak drop all pass.

## Known limits, left as they are

- At start, a declined invitation is matched by its group id only, because its root is not known there yet.
- A device that only just started may follow a move for a moment before core's checks are in place.
- Once a move is held, the old group waits until the next start or until its new log is taken.
- A move declined more than 30 days ago and then accepted does not carry over the old name.
- When two groups move to the same log, the second is never noted. This is harmless.

## Not in this change

`RAISED_TO` goes to five only once these land:

1. The move button, the fill warning, and the app following `GroupMoved` (ffi and Flutter).
2. Offers and joins carrying only needed entries.
3. A level-five corpus next to the level two to four ones.
4. A network lab scenario of a group moving.
