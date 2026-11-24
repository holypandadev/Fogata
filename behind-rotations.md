Branch: lab/behind-rotations at aa263ab, on main 49c97a6. No merges. Independent of the level-five branches.

Before: when a member makes two key changes while others are away (for example, removing two members), those others come back knowing only the key from before both. They announce and look only under that key. The member that made the changes looked them up there but never announced there, and behind NATs that filter by address and port a dial from one end alone never gets through. So it never reached them, and they stayed apart from it.

After: members announce under the two older keys they look up as well, for the same two weeks after the key that replaced each began. Both ends find each other there and the hole punch works.

## How

- `refresh_announcement` (`node/dht.rs`) announces under `GroupContext::lookup_secrets` instead of `rendezvous_secrets`. Joining devices and non-members get the same keys as before.
- Doc comments in `group.rs`; `docs/architecture/3-connectivity.md` explains why both ends must announce, and what a removed device that kept an older key can see: members' announcements there for two weeks, as the device removed by the latest change already does under the previous key. It could already learn their addresses by announcing there itself, and it still cannot prove itself a member.
- Lab: `group-behind` in `tools/netlab/netlab.sh`, in the default list, timed in `shards.py`. A, B and C, each behind a cone NAT, are in a group with D and E; A removes D and E while B and C are away; then the three meet, and the messages A and B send must reach the other two.

## Proof

- fogata-net `node/dht.rs`: `a_member_is_announced_where_members_two_rotations_behind_look` (announced under the oldest of three keys now, and not 15 days on, while the newest stays announced). Fails without the change.
- Network lab, run locally: `group-behind` fails without the change (A finds B and C but never connects; B and C meet each other only) and passes with it (A connects to both, every message reaches the other two).
- `cargo fmt --check` and `cargo clippy -D warnings` on fogata-net (all targets): clean. fogata-net: every test passes but `files::tests::what_an_earlier_run_left_and_cannot_go_is_left`, which needs a folder to be read-only and fails the same way on main on the test machine (root ignores it); it runs in CI.
