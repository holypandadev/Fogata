# turn in Fogata

This is turn 0.17.2 from crates.io (MIT or Apache-2.0, see LICENSE-MIT and LICENSE-APACHE),
the TURN server the media bridge (`crates/fogata-media`) runs for the local WebRTC engine,
without its examples, benchmarks and dev-dependencies, so its own tests run offline from
the workspace (`cargo test -p turn --lib`, in tools/check.sh): one test no longer starts
env_logger, two decode hex with a few lines of their own instead of the hex crate, and the
two client tests that ask a public STUN server on the internet
(`test_client_with_stun_send_binding_request` and its `_to_parallel` twin, in
src/client/client_test.rs) are ignored. The
workspace takes it through `[patch.crates-io]`. Every change from the published crate is
listed here.

## Nonces never used (src/server/request.rs)

The server answers every request without valid credentials with a new nonce and keeps it,
until a request uses it once past its lifetime (an hour): a nonce never used stayed for as
long as the server ran, so anything that could send requests to it grew its memory without
end. From 1024 nonces kept on (`NONCES_PURGED_AT`), those past their lifetime go before a
new one is kept, and the oldest go while there are 4096 (`MAX_NONCES`); a client whose
nonce went gets a new one with "stale nonce" and asks again, as for one past its lifetime.
Tested by `test_nonces_never_used_are_bounded` (src/server/request/request_test.rs).

## Whose allocation it is (src/relay/mod.rs, src/allocation/allocation_manager.rs)

`RelayAddressGenerator` has one more method, `allocate_conn_for`, which is also told the
user the allocation's request was authenticated as; by default it does what
`allocate_conn` does, and the allocation manager calls it when it makes an allocation.
Fogata's media bridge names in that user the member a connection of the local WebRTC
engine is for, and sends what the allocation relays to that member only. Tested by
`test_relay_address_generator_is_told_the_username`
(src/allocation/allocation_manager/allocation_manager_test.rs).
