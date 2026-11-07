# Fuzzing

Everything Fogata reads from outside goes through a decoder: what other devices send (group
log entries and logs, member and friend requests and answers, room events, membership
proofs, typing signals, media packets and their parts, packets to the TURN endpoint, direct
messages), what people paste (group invites, friend links and codes), the signed records
passed around (profiles, banners, bios, animations, group banners), and what the device
keeps on its disk (sealed files, chat journals, friends file, group logs). Fuzzing feeds
each of them damaged and made-up inputs, looking for any that make it panic, hang or use too
much memory. A panic in the node would stop it for every group and friend, so a decoder must
refuse what it cannot read, never crash on it.

## The targets

`crates/fogata-core/src/fuzzing.rs` lists every target (`TARGETS`): its name, the function
that reads the bytes as Fogata does, and well-formed seeds to start from. The functions of
other crates live in their own `fuzzing` modules (`fogata-net`, `fogata-media`,
`fogata-friends`, `fogata-voice`), built only with the `fuzzing` feature, so nothing of it is in the
application. Besides not panicking, a target checks what must hold for anything that reads:
what decodes writes and decodes back the same; a group log makes the same group whatever
order its entries come in; a packet split in parts comes back whole.

| Target | What it reads |
|---|---|
| `entry` | A group log entry, an invite ticket |
| `log` | A group log, taken in as a member does, then what the node asks of the group |
| `proof` | Sealed membership proofs, boxes sealed to the device, group tags |
| `member_wire` | Member requests and answers off the wire (CBOR), and their fields |
| `node_request` | Member requests handled by a running node against its real state (a group, a member, a stranger) |
| `node_join` | Sequences of member requests, and answers to its asks, from a stranger and three managers, against a node joining a group with an invite; what it holds of logs in parts must stay within its bounds |
| `node_friend` | Sequences of friend requests from a stranger and two friends (one on two connections) against a node with friends; what offers in parts hold must stay within their bound |
| `friend_wire` | Friend requests and answers off the wire |
| `friend_sealed` | What a friend seals to the device: hellos, profiles, statuses, banners, bios, animations, calls, friends in common, acceptances |
| `room_event` | Room events (joins, chat, shares, removals...) applied to a voice room |
| `parts` | Media datagrams and their parts, put back together |
| `turn` | Packets to the TURN endpoint, handled by a TURN server like the bridge's |
| `direct_payload` | Direct messages, receipts, timers and pins, taken into a chat |
| `typing` | Typing signals |
| `sealed_name` | Sealed local addresses in DHT announcements |
| `invite` | Group invites, friend links and friend codes as people paste them |
| `records` | Group banners, animations, profiles, banners, bios, the friends file |
| `at_rest` | Sealed files, chat journals (sealed and plain), sealed attachments, group log files |
| `mls` | Log entries of groups on MLS (and the chain worked out with them), KeyPackages, a device's MLS state file, Commits followed and Welcomes joined with |
| `ogg_read` | A friend's voice message file (Ogg Opus), as it came and with its checksums fixed; one that reads is written again and must read back the same |
| `voice_play` | A voice message played to its end at two speeds the first byte chooses, within as many steps as its length allows at the slowest |
| `wsola_step` | Voice stretched in time from only what each step says it needs, which must be what stretching from all of it gives |
| `resample` | Sound changed between two rates, whole and in pieces |
| `recorder` | Sound from a microphone, any values, encoded; the file made must read back with its length |

## Long runs, with the stable toolchain

Each target also runs as an ignored test of fogata-core that damages its seeds again and
again (bits flipped, bytes and lengths changed, parts cut, copied or taken from another
input) for as long as it is told:

    tools/fuzz.sh 10                 # every target for 10 minutes, 4 at a time
    JOBS=2 tools/fuzz.sh 30 log turn # two targets for 30 minutes

`tools/check.sh` runs every target on its seeds and a few hundred damaged copies, so the
targets keep working as the code changes. An input that panics or takes more than five
seconds is saved under `target/fuzz-found/<target>/`, named by the run's seed (printed at
the start) and its number.

## Coverage-guided, with libFuzzer

`fuzz/` holds one libFuzzer target per entry of `TARGETS`. It is a workspace of its own, with
its own `Cargo.lock`: the main workspace, `tools/check.sh` and `cargo deny` never see it or
`libfuzzer-sys`. Its `[patch.crates-io]` must name every patch of the main `Cargo.toml` (the
vendored `libp2p-quic`, `libp2p-mdns`, `turn` and `spake2`), or the targets would not
compile, or would fuzz other code than the app ships: `tools/check.sh` fails when the two
lists differ, and `tools/ci.sh` builds the targets (`cargo +nightly fuzz build`) without
running them. It needs the nightly toolchain and cargo-fuzz, installed once:

    rustup toolchain install nightly --profile minimal
    cargo install cargo-fuzz

Then:

    tools/fuzz.sh --libfuzzer 20             # every target for 20 minutes, 4 at a time
    JOBS=6 tools/fuzz.sh --libfuzzer 60 log  # one target for an hour

It writes the seeds to `fuzz/seeds/`, builds the targets, and runs each with a 10-second
limit per input and 2 GB of memory. What libFuzzer learns stays in `fuzz/corpus/<target>/`
for the next run; logs go to `fuzz/logs/`. A finding lands in `fuzz/artifacts/<target>/`
and the script says so. To run one by hand, or to replay or shrink a finding:

    cd fuzz
    cargo +nightly fuzz run --fuzz-dir . log corpus/log seeds/log
    cargo +nightly fuzz run --fuzz-dir . log artifacts/log/crash-...
    cargo +nightly fuzz tmin --fuzz-dir . log artifacts/log/crash-...

Every finding fixed gets a test seeded from its (shrunk) input, next to the code it fixes.

## Adding a target

Write the function (and its seeds) next to the decoder, behind the `fuzzing` feature when
it needs the crate's private items; add it to `TARGETS` and to the `long_runs!` list in
`crates/fogata-core/src/fuzzing.rs`; add `fuzz/fuzz_targets/<name>.rs` and its `[[bin]]` in
`fuzz/Cargo.toml`. `tools/check.sh` fails until the libFuzzer target exists.
