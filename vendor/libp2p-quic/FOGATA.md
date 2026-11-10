# libp2p-quic in Fogata

A copy of libp2p-quic 0.14.0 (MIT, see LICENSE) from crates.io, put in place of the published
crate by `[patch.crates-io]` in the workspace's Cargo.toml (and fuzz/Cargo.toml). Its
integration tests (`tests/smoke.rs`, `tests/stream_compliance.rs`) and dev-dependencies are
left out; the unit tests in `src/` stay and run in `tools/check.sh` (`cargo test -p
libp2p-quic --lib`). `tools/vendor_diff.py libp2p-quic` shows every difference from the
published crate. Only these changes (docs/architecture.md, the measurements of media over
QUIC and the bounds of incoming connections, says why):

1. Unreliable datagrams (RFC 9221) for real-time media. rust-libp2p's muxer carries reliable
   streams only and the published crate turns datagrams off; media needs packets that may be
   lost instead of holding back the ones behind them.
   - `src/config.rs`: datagrams on, with a 1 MiB receive buffer and a 64 KiB send buffer (a
     small one drops stale media instead of sending it late); BBR as the congestion
     controller in place of Cubic, which took random loss on Wi-Fi and mobile links for
     congestion and queued video.
   - `src/datagram.rs` (new): every established connection is registered by its remote
     peer (`src/connection/connecting.rs` calls `register` once the handshake is done).
     `receive(capacity, accept)` gives the datagrams of every connection, tagged with the
     peer, in one bounded queue: only from the peers `accept` takes (any peer that
     completes a handshake, a DHT node or a stranger, can send datagrams), at most
     `PEER_SHARE` of one peer's waiting, and a queue dropped lets go of its `accept`.
     `link(peer)` is the connection to send on (the one that received a datagram last,
     unless that was more than 2 s ago, else the newest: after a network change the old
     one lingers until its idle timeout), `silent_for(peer)` how long the peer's
     connections have heard nothing, and `DatagramLink` sends, says the largest datagram
     that fits and how the path is doing (`PathState`). Tests:
     `a_connection_is_silent_from_the_last_time_its_count_moved`,
     `datagrams_the_application_does_not_take_are_dropped`,
     `a_flood_from_a_stranger_or_one_peer_leaves_room_for_the_others`,
     `a_queue_dropped_lets_go_of_its_accept`.
   - `src/lib.rs`: `pub mod datagram`.
   - `Cargo.toml`: `bytes`, which the datagrams are carried in.
2. A flood of first packets with made-up source addresses could hold any number of
   handshakes. `src/config.rs`: `Config::retry_above` (64): while that many incoming
   connections or more are being set up, a new one from an address QUIC has not seen proven
   is answered with a retry packet (RFC 9000, section 8.1.2), which costs no state and which
   only a sender at a real address answers. `src/transport.rs` counts the incoming
   connections being set up over every listener and sends the retries;
   `src/connection/connecting.rs` keeps an incoming connection in that count until it is
   established, fails or is dropped. Test:
   `an_unproven_address_past_the_limit_is_retried_and_then_accepted`.
3. `Cargo.toml`: a note only, on why quinn stays on aws-lc-rs (libp2p-tls uses it for the
   post-quantum X25519MLKEM768 exchange, which ring lacks).

To move to a new upstream version: download it, apply these changes again, keep this list
true, and run `tools/vendor_diff.py libp2p-quic` to see that nothing else differs.
