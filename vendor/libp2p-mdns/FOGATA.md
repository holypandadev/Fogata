# libp2p-mdns in Fogata

A copy of libp2p-mdns 0.49.0 (MIT, see LICENSE) from crates.io, put in place of the published
crate by `[patch.crates-io]` in the workspace's Cargo.toml. Only these changes:

1. `src/behaviour/timer.rs`: the one-shot timer for the next expiry was a tokio interval with
   a near-endless period; polled more than 5 ms late (a busy computer, one back from sleep),
   tokio added that period to the current time, which overflowed and panicked the network
   task. Its period is a year now. Test: `a_timer_polled_late_fires`.
2. Anyone on the local network could make the node keep any number of made-up peers, each for
   as long as it liked: a response was taken whether or not anyone had asked, the peers it
   named were kept in a list that only grew and was searched from end to end, and their
   expiry was the TTL the response gave (up to 136 years).
   - `src/behaviour/iface.rs`: a response is taken only when it answers one of this
     interface's own recent queries (by its id; 30 seconds since 5. below) and comes from the
     interface's network or a link-local address (RFC 6762, section 11). What a response
     tells is kept at most twice the query interval (Fogata asks every 30 seconds), and at
     most 256 addresses wait to be handed on. Test:
     `only_responses_to_a_recent_own_query_from_the_network_are_taken`.
   - `src/behaviour/iface/query.rs`: a response carries its id, and its TTL is cut to that
     longest time.
   - `src/behaviour/found.rs` (new) and `src/behaviour.rs`: the discovered peers are kept in
     a map, at most 128 in all, 16 told of by one address and 8 addresses each. A new peer
     past the limit of its source takes the place of that source's peer that expires
     first; past the limit in all, that of the source with the most. The addresses dropped
     to make room are reported as expired. A peer's source is the address whose response
     first named it, until a connection is made at an address another response told of
     (`src/behaviour.rs` notes each connection this node dials): that response's address
     is the source from then on. Another address can add only addresses of its own to that
     peer (the same computer answering from a second interface, or the peer itself after a
     neighbour named it first), in the place of one never reached that another address
     told of; an address reached is never dropped for another. So a neighbour can neither
     count someone else's peer among its own share and push it out, nor fill its addresses
     with made-up ones, nor, by answering first, keep the peer's own answer out.
     `Behaviour::told_by` gives the node the address of the response that told of each
     address, which the node counts its dial by. Tests:
     `a_flood_of_made_up_peers_keeps_a_bounded_number`,
     `addresses_expire_and_dropped_ones_are_told`,
     `a_neighbour_cannot_speak_for_a_peer_another_address_told_of`,
     `the_first_to_answer_for_a_peer_does_not_keep_it`.

   A device on the same network still finds the others at its first queries, which they
   answer at once, and the others find it at their next query (every 30 seconds in Fogata):
   only the responses to someone else's query are no longer taken. The node limits further
   how many new peers it dials (fogata-net, `Limits::take_mdns_peer`).

   What the query id does and does not do: it is 16 bits sent in the clear to the multicast
   group, so anyone on the local network reads it and can answer anything to it. It keeps
   out responses from the internet (which also fail the network check) and unasked floods
   from the local network; what bounds a neighbour that answers the queries are the limits
   above and the node's. `Behaviour::told_by` gives the node the address of the response
   that told of an address, which the node counts new peers by: the addresses a response
   names are whatever its sender likes.

3. `src/behaviour/iface.rs`: a response no longer ends the first queries, sent at doubling
   intervals from half a second up to the query interval. The first to answer may be any
   device on the network, started long before, while the one being looked for, starting at
   the same time, was not answering yet or its answer was lost: the next query was then a
   whole interval (30 seconds in Fogata) away. Test: netlab `mdns-stale`.

4. `src/behaviour/iface.rs`: each query sent (with its id), each query answered (with how
   many addresses) and each response taken or left out (with how long after its query) is
   said at debug, to see where a slow discovery loses its time (`FOGATA_TRACE` in
   crates/fogata-net/tests/mdns_stale.rs).

5. On macOS the application firewall checks, in its daemon and one after the other, every
   new sender each socket of the program hears from, also for an allowed program: about 0.4
   to 0.85 seconds each for a test binary signed ad hoc, more on a busy computer (seen in the
   system log: `socketfilterfw` "Handle flow" then "KNOWN APP FLOW"). Every copy sent from
   a new port of its own, so each copy that started was a new sender for every other copy's
   socket, and the answers to the first queries came seconds late, past the five seconds a
   response was taken in, and were dropped (`mdns_stale` on a Mac: 4 to 16 seconds idle,
   none within 25 under load).
   - `src/behaviour/iface.rs`: over IPv4 queries and answers go from the socket bound to
     port 5353, on its interface (`IP_MULTICAST_IF`; should the system refuse it, the
     routes pick one and the interface is kept), as RFC 6762 has responders do: every copy
     on a computer sends from the same address and port, met once by each socket. On Linux
     the socket also has `IP_MULTICAST_ALL` off, so it hears only its own link (Linux hands
     a socket the group's packets from every interface any socket joined it on).
     IPv6 keeps its own sending socket (Fogata does not run mDNS over IPv6).
   - A response is taken up to 30 seconds (one query interval of Fogata's) after the query
     it answers, not 5.

6. `src/behaviour.rs`: the tasks of the interfaces stop when the `Behaviour` is dropped.
   Upstream they went on after their node stopped, answering every query with its dead
   addresses until their own next query (up to a query interval later): every device on
   the network dialled them for nothing (the `mdns_stale` rounds on a Mac each dialled
   the copies of the round before). Test: `the_interfaces_stop_with_the_behaviour` (the
   crate builds only the tokio provider). Beside it, fogata-net dials again, twice, a peer
   mDNS found whose dials all failed, at the addresses its own answers gave (sent from
   them) and on that answer's share of new peers, never for an answer from an address not
   heard now (`MDNS_RETRIES` in node.rs, tests
   `a_peer_mdns_found_whose_dials_failed_is_dialled_again_a_few_times` and
   `retries_are_only_for_a_peer_s_own_answer_and_end`).

7. `src/behaviour.rs`: what the interfaces told is taken at most 16 addresses at a time
   (`found::MAX_PEERS_PER_SOURCE`), and what it told of is handed on before the next lot is
   taken. A new peer past its address's share takes the place of that address's peer that
   expires first, and a peer told of a moment ago is that one once every other of its
   address was heard again after it: copies on one computer, more than 16, all answer each
   query at once, and taken all in one go the answers right after a new one pushed it out
   before the node asked who told of it (`Behaviour::told_by`), which then dialled it not
   at all until its next answer. Test: `a_peer_told_of_is_known_when_that_is_handed_on`
   (with a provider whose interfaces never come up, so no socket is opened).
   Beside it, fogata-net lets the peers that no share of new peers could take wait for one,
   in turn (`MAX_MDNS_WAITING` in node/mdns_dials.rs): a peer is told of only once.

The crate is not a workspace member: `tools/check.sh` runs its tests with
`cargo test -p libp2p-mdns --lib`.
