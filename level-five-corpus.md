Branch: lab/level-five-corpus at 71598bc, on top of lab/move-past-own (8a64fa5). No merges.

Before: levels two to four have a corpus that pins how every copy judges a stored log, so a change that judges old logs differently fails a test. Level five has none.

After: `level-five.bin` and `level-five.txt` pin six level-five worlds: floods stopped at the ceiling, removals past it, a member writing apart on stale heads, a forged move, raises to six, and the move itself (four of the six worlds move). Each world is stored as written and again with entries like each one added. The text also prints the fill, the move and every refusal with its reason.

## How

- `log/tests/level.rs`: rooms (run and ceiling) per world, the level-five lines in the judgment, `the_judgment_of_level_five_never_changes`, and the ignored `write_the_level_five_corpus` (run with `FOGATA_LEVEL_FIVE_CORPUS`), which asserts that the corpus holds entries refused past the ceiling.
- `ceiling.rs`: two helpers made visible to the corpus.

## Proof

- `the_judgment_of_level_five_never_changes` passes on this commit and fails if any rule of level five changes how a stored log is judged (checked: judged without the move fix of lab/move-past-own, this corpus fails the test).
- `FOGATA_LADDER_SEEDS=40 cargo test --release -p fogata-state`: 233 passed. `cargo fmt --check` and `cargo clippy -D warnings`: clean.
