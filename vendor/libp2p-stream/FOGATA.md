# libp2p-stream in Fogata

A copy of libp2p-stream 0.5.0-alpha (MIT, see LICENSE: rust-libp2p's, which the published
crate does not carry) from crates.io, put in place of the published crate by
`[patch.crates-io]` in the workspace's Cargo.toml and fuzz/Cargo.toml. Its tests
(`tests/lib.rs`) and dev-dependencies are left out, and its changelog. Only this change:

1. `src/shared.rs`: `Control::accept` handed inbound streams to the application through a
   channel with room for one (`mpsc::channel(0)`: no buffer, one slot for its one sender),
   and dropped every stream that came while it was full ("Channel is full, dropping inbound
   stream", at debug). Streams of one protocol that came in together, before the task
   taking them ran again, were all lost but one, and their askers saw them end or reset
   before any answer (`UnexpectedEof`, `ConnectionReset` with `Stopped(0)`). On a loaded
   computer that is several previews of a room's pictures, several downloads, update asks,
   typing signals or the media bridge's streams from one member at once. The channel now
   keeps up to `INBOUND_QUEUED` (128) streams of each protocol, and the one slot of its
   sender; past them, a stream is still dropped as before.

   Why 128: enough for an honest burst, which is a room page asking for all its previews
   at once (a file server takes up to 128 requests at once from one device and 512 from
   all, `MAX_REQUESTS_PER_PEER` and `MAX_REQUESTS` in crates/fogata-net/src/files.rs), and
   as many as libp2p negotiates at once on one connection (`max_negotiating_inbound_streams`,
   128 by default), so one connection's whole burst fits. It is no lever for a hostile
   peer: what waits is a handle (the peer and a stream) of a few hundred bytes, at most 128
   per protocol and a dozen protocols; the stream itself is open whether it waits here or
   in the task that takes it, it counts against its connection's 256 streams at once
   (`max_concurrent_stream_limit` of QUIC, `MAX_STREAMS` for yamux in
   crates/fogata-net/src/node.rs), and what can be sent on it before it is read is held by
   the transport's own windows (QUIC: 10 MB per stream, 15 MB per connection). Each loop
   taking streams applies its own limits right after it takes one (members only, so many
   at once per device), and drops the stream when they are reached.

   Tests: `streams_opened_at_once_all_reach_a_late_taker` (crates/fogata-net/tests/bursts.rs)
   and `files_asked_at_once_of_a_busy_server_are_all_answered` (crates/fogata-net/src/files.rs),
   which fail with the published crate: 1 of 48 streams reaches the taker, 1 of 4 requests
   is answered. Copies of Fogata before 0.1.0-beta.9 keep the published crate, so a file
   request dropped unanswered is made again a few times (`ASKED_AGAIN_AFTER` in files.rs).
