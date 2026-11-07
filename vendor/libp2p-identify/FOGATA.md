# libp2p-identify in Fogata

A copy of libp2p-identify 0.48.0 (MIT, see LICENSE) from crates.io, put in place of the
published crate by `[patch.crates-io]` in the workspace's Cargo.toml (and fuzz/Cargo.toml).
Its tests (`tests/smoke.rs`) and dev-dependencies are left out. Only these changes:

1. A device told every peer that connected all the protocols it takes, Fogata's own among
   them (`/fogata/member/1`, `/fogata/caps/2`...): every DHT node a lookup passes through
   learnt that the device runs Fogata, and its version. Upstream identify has no way to list
   some protocols to some peers only.
   - `src/behaviour.rs`: `Config::with_protocols_shown(prefix, to)`: the protocols whose
     names start with `prefix` are listed only to the peers `to` takes. The handlers get it.
   - `src/handler.rs`: each time the handler says who this node is (an answer, a push), it
     asks `to` about its peer until `to` takes it; from then on that connection lists them
     for good, so a later identify on it never takes them back. A change of the protocols
     taken that changes nothing a connection lists is not pushed on it.

   Fogata lists `/fogata/` protocols to the peers it wants (chose to reach, or that proved
   something) and to members and friends while they stay connected, and pushes to a peer
   once it proves itself (`crates/fogata-net/src/behaviour.rs`, `Node::show_protocols`).
   Tests: `identify_does_not_name_fogata` (behaviour.rs) and
   `fogata_protocols_are_listed_to_a_peer_once_it_proved_itself` (node.rs) in fogata-net,
   `members_learn_each_others_level_and_never_take_it_lower` (crates/fogata-net/tests/levels.rs).

2. Pushes came in bursts of hundreds within milliseconds, and the other end dropped all but
   a few, warning of each (`Dropping inbound identify push stream because we are at
   capacity`): upstream pushes to every connected peer on every new or expired listen
   address, even with listen addresses hidden, when the push says nothing new, and opens a
   push per change on a connection with no limit. A network joined or left, a sleep ended,
   temporary IPv6 addresses, interfaces of virtual networks and slots at members' relays
   each give one; a connection takes ten streams at once.
   - `src/behaviour.rs`: with `with_push_listen_addr_updates`, a push goes only when the
     addresses said (`all_addresses`) change, listen or external.
   - `src/handler.rs` (`Pushes`): one push open at a time per connection; any asked for
     meanwhile are folded into one more, sent when it ends (sent, failed or timed out). A
     push for a change is left out when the connection would say what it said last (in an
     answer or a push that got there; one that failed or timed out leaves it unknown; of
     two that end out of order, the one built last counts); one
     asked by `Behaviour::push` goes anyway. An inbound stream
     dropped for want of room is warned of once per connection, then logged at debug.

   Tests: `pushes_on_a_connection_are_one_at_a_time`,
   `a_push_says_something_new_unless_asked`, `a_failed_answer_leaves_the_next_change_pushed`
   and `a_push_that_times_out_or_fails_ends` (`cargo test -p libp2p-identify --lib`, in
   tools/check.sh; the published crate's dev-dependencies are left out, since a crate
   outside the workspace cannot be tested with them), `a_burst_of_changes_is_pushed_in_a_push_or_two`
   and `members_push_to_each_other_a_bounded_number_of_times` (node.rs) in fogata-net.
