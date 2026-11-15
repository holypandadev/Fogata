# libp2p-quic in Fogata

A copy of libp2p-quic 0.14.0 (MIT, see LICENSE) from crates.io, put in place of the published
crate by `[patch.crates-io]` in the workspace's Cargo.toml (and fuzz/Cargo.toml). Its
integration tests (`tests/smoke.rs`, `tests/stream_compliance.rs`) and dev-dependencies are
left out; the unit tests in `src/` stay and run in `tools/check.sh` (`cargo test -p
libp2p-quic --lib`). `tools/vendor_diff.py libp2p-quic` shows every difference from the
published crate. Only these changes (docs/architecture.md, the measurements of media over
QUIC in 4.2 and the bounds of incoming connections in 3.2, says why):

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
4. On Linux a packet to an address on the local network that no host answers for waits in
   the kernel for the neighbour to be found (up to one default send buffer of packets for
   each such address, dropped after three seconds) and counts against the socket's send
   buffer meanwhile. Dials to some 90 made-up peers at one such address filled the default
   buffer, and the socket sent nothing to any peer for seconds. `src/transport.rs`: every
   socket asks for a 2 MiB send buffer on Linux (`SEND_BUFFER`; the kernel gives what
   `net.core.wmem_max` allows, with its default twice the default buffer, which packets
   to three or more such addresses could still fill: change 5 bounds those). Test:
   `the_send_buffer_has_room_past_the_packets_one_silent_address_holds`.
5. Dials to addresses of the device's own networks take turns, so that whatever peers a DHT
   node makes up at addresses there where no host is, the packets that wait for those
   neighbours take a bounded part of the send buffer, and a dial the node vouched for is
   never kept waiting by them. `src/on_link.rs` (new, a public module): `DialRoom` gives
   each such dial a share of what may wait (14 KiB for a dial, 32 KiB for a hole punch),
   held while the dial runs and for 3.5 s after unless its address answered, as its last
   packets may wait that long. The room is in two parts: `vouch(ip)`, which the node calls
   just before it dials an address it has a reason of its own to (mDNS, a member's or
   friend's addresses, a punch, an address the person gave), notes the address in a
   process-wide registry (as `src/datagram.rs` keeps its own) for `VOUCHED_FOR` (60 s),
   256 addresses at most, the oldest going first; dials to those take their turns in
   `VOUCHED_IN_ALL` (64 KiB), and every other dial in `IN_ALL` (112 KiB, eight dials), which
   can never take the first. Each part holds 32 KiB for one address at most; both together
   are 42% of what the kernel gives with `net.core.wmem_max` at its default, and under the
   default buffer without change 4 (the notes at the top of the file work the figures out
   from how Linux holds those packets). In each part the others wait their turn, the one
   whose address holds least first, within the time the dial has (`handshake_timeout`,
   which the wait counts in). An address that set up a connection,
   while it is open and for ten minutes after, needs no turn, nor does the loopback or any
   address off those networks. `src/transport.rs`: each listener keeps its interfaces'
   networks from the interface watcher, and `dial` waits for the turn before the first
   packet of a dial or a punch; `src/connection/connecting.rs` and `src/connection.rs` note
   the remote address of every connection set up (incoming or outgoing) as one that answers
   until it closes. Tests: `dials_to_addresses_that_have_not_answered_wait_for_room`,
   `a_dial_whose_address_answered_gives_its_room_back_at_once`,
   `a_punch_is_not_passed_by_later_dials`, `an_address_that_holds_nothing_goes_first`,
   `a_dial_that_goes_away_leaves_no_room_taken`, `an_address_that_answered_needs_no_turn`,
   `a_waiting_dial_is_woken_when_lingering_room_comes_back`,
   `a_vouched_dial_goes_at_once_while_the_others_are_full`,
   `dials_not_vouched_for_never_take_vouched_room`, `each_part_keeps_one_address_to_its_share`,
   `vouches_expire_and_are_bounded`, `vouching_puts_an_address_in_the_vouched_part`,
   `dials_to_the_devices_own_networks_take_turns`; the network lab's `media-silent` and
   `lan-first-contact`.

To move to a new upstream version: download it, apply these changes again, keep this list
true, and run `tools/vendor_diff.py libp2p-quic` to see that nothing else differs.
