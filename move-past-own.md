Branch: lab/move-past-own at 8a64fa5, on top of lab/log-follow (5751119). No merges.

Before: the first member writes a change (for example a rename) before a flood by two other devices has reached it, then removes the flooders and moves the group to a fresh log. Once the rest of the flood arrives, the common order puts that change past the ceiling, and the move, signed on it, is refused on every copy, the first member's included. The first member has already started the new log, while everyone else stays on the full one, which can no longer move.

After: the first move may be signed on an entry of its own author's that the order puts past the ceiling, as a removal or leave already may. The move counts the same on every copy, whatever arrives after it.

## How

- `budget.rs`, `charge`: the rule that refuses an entry signed on its author's own entry past the ceiling now lets through the first counted `Moved`, as it lets through what gets the group out.
- Docs: the slack and reserve paragraph and the list of tests in `docs/architecture/5-group-management.md`.

## Proof

- fogata-state, `ceiling.rs`: `a_move_on_a_change_the_flood_pushed_past_the_ceiling_counts`. Two devices flood; the first member renames before the flood reaches it, removes them and moves; the rest of the flood arrives. On the mover's copy and on a copy with everything, the rename is past the ceiling, the move counts, and the new log follows. Without the fix the move is refused on both copies. 100 runs in a row with the fix: no failure.
- `cargo fmt --check` and `cargo clippy -D warnings`: clean. `FOGATA_LADDER_SEEDS=40 cargo test --release -p fogata-state`: 232 passed.

## Known limit, left as it is

What a move brings in that is part of no run still counts against the 256-entry slack. Such a move is refused only if removals and leaves have already used the whole slack, which no test here reaches.
