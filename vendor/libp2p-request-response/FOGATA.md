# libp2p-request-response in Fogata

A copy of libp2p-request-response 0.30.0 (MIT, see LICENSE: rust-libp2p's, which the
published crate does not carry) from crates.io, put in place of the published crate by
`[patch.crates-io]` in the workspace's Cargo.toml and fuzz/Cargo.toml. Its integration tests
(`tests/`), its dev-dependencies and its changelog are left out. The unit tests of its codecs
stay in `src/cbor.rs` and `src/json.rs` as published, but are not run: they need the
dev-dependencies left out, and Fogata uses neither codec but in its tests.
Only this change:

1. `src/lib.rs`: `Behaviour::send_request_on` sends a request on a given connection to the
   peer, while the behaviour still counts it open; otherwise on the one `send_request` takes,
   and with no connection at all it dials and waits as `send_request` does. Upstream offers
   no way to choose: a request goes on `connections[request_id % len]`, so with several
   connections to one peer (TCP, the QUIC one dialled for media, a relayed one kept a few
   seconds, a silent direct one being left) requests took them in turn, the slower ones and
   those about to close included. Fogata sends each member request on the peer's best
   connection that is not suspect (`request_connection` in
   crates/fogata-net/src/node/links.rs). Nothing changes on the wire.

   A connection that died unnoticed (after an outage, or a sleep: an old one ranks before
   one dialled anew on the same path) is still chosen until it is suspect: 3 s after a small
   request on it went unanswered (`SUSPECT_AFTER` in crates/fogata-net/src/node/outbox.rs),
   or, a direct QUIC one, once the most recently heard of the peer's direct QUIC connections
   has received nothing for 6 s: silence is counted per peer, so an old dead QUIC connection
   beside a live new one is caught only by the 3 s rule. Requests sent on it before then are
   lost, at most the 6 on their way to that peer at once; each fails after 10 s (its stream
   does not open). Without the guard it is chosen until it is known dead: a QUIC connection
   closes 25 to 30 s after it died, a TCP one 55 to 70 s after (derived from libp2p-ping's
   timers in the `Ping` arm of `on_behaviour_event`, crates/fogata-net/src/node/swarm_events.rs).

   Tests: `signals_in_a_row_all_go_on_the_best_connection`, which fails with the published
   crate's choice (the signals come on both connections in turn),
   `signals_go_on_the_connection_left_once_the_best_one_closed`,
   `a_request_on_a_closed_connection_goes_on_another`, and for the suspect connections
   `signals_leave_a_best_connection_that_stopped_answering`,
   `a_large_slow_request_does_not_make_the_best_connection_suspect` and
   `signals_go_on_the_best_connection_when_every_one_is_suspect`
   (crates/fogata-net/tests/signals.rs).
