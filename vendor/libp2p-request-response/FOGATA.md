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
   seconds, a silent direct one being left, a dead one after an outage) requests took them
   in turn, the slower ones and those about to close included, and one on a dead connection
   waited the whole request timeout (20 s in Fogata) for nothing. Fogata sends each member
   request on the peer's best connection (`best_connection` in
   crates/fogata-net/src/node/links.rs). Nothing changes on the wire.

   Tests: `signals_in_a_row_all_go_on_the_best_connection`, which fails with the published
   crate's choice (the signals come on both connections in turn),
   `signals_go_on_the_connection_left_once_the_best_one_closed` and
   `a_request_on_a_closed_connection_goes_on_another` (crates/fogata-net/tests/signals.rs).
