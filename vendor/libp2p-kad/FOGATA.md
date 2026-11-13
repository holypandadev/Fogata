# libp2p-kad in Fogata

A copy of libp2p-kad 0.49.0 (MIT, see LICENSE: rust-libp2p's, which the published crate does
not carry) from crates.io, put in place of the published crate by `[patch.crates-io]` in the
workspace's Cargo.toml and fuzz/Cargo.toml. Its integration test (`tests/client_mode.rs`), its
dev-dependencies and its changelog are left out. Its unit tests stay in `src/` as published,
but they cannot be built from the published crate (they need `quickcheck` and
`libp2p-swarm-test` from the rust-libp2p tree, which the published crate does not name): they
are run in rust-libp2p at the commit the crate was published from
(`7171dce2f90c05ba7892d4ba926abb1881db27c7`, `protocols/kad`), with Fogata's changes applied.

Only this change:

1. `src/behaviour.rs` (`HandlerEvent::GetProvidersRes`): upstream handed the providers an
   answer names to the lookup's iterator together with the closer peers, so a lookup walked
   them as if they were DHT nodes: each one nearer the key than the nodes not asked yet was
   dialled at the addresses its record gave and held one of the lookup's places (3, the
   parallelism) until it answered or `peer_timeout` (10 s) went by. A node on the way to a
   key can name any number of made-up providers, new ones for each answer, at an address
   that never answers; those nearer the key than the nodes that keep the real announcement
   held every place, ten seconds at a time, and the real answer came only once they were
   over. In the network lab (`dht-liars`) two members found each other 14 s after a lookup
   that a lie took first, instead of 3 s. Providers are no DHT nodes in any case (Fogata
   devices, the providers it looks for, run the DHT as clients and answer no request).
   Now only the closer peers go on into the iterator. The providers' addresses are still
   kept in the query's addresses, as upstream did, so a dial to a provider while its query
   runs (Fogata dials the members it finds at once, `Node::dial_found`) finds them.

   The answers to the other requests (`FIND_NODE`, `GET_VALUE`) hand the iterator only the
   closer peers already. A node on the way can still name made-up closer peers in any
   answer; that is how a Kademlia lookup works, and those are asked as nodes only while
   they are among the nearest the lookup knows, as for every lookup on the public DHT.

   Test: `a_lookup_asks_the_nodes_an_answer_names_not_the_providers_it_names`
   (crates/fogata-net/tests/lookups.rs).
