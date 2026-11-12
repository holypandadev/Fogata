Part of [Fogata architecture](../architecture.md).

## 8. Testing

- Unit tests for the crypto and wire formats.
- Integration tests of each crate in its `tests/` folder, through its public calls only:
  for fogata-ffi, the main things a person does through the calls the screens make, with no
  Flutter (`tests/smoke.rs`: a profile, a group joined with an invite, a friend made from
  it, a direct chat with files, a voice room with its chat, signals and files, a restart
  on the same folder, as the screens get them: the bridge's types, its events and the
  words of its errors), one core through the bridge and the other on its own, over the
  loopback interface; and the parts of the core's error messages the app explains in its
  own words (`app/test/core_errors.txt`), which the core must still write
  (`tests/error_texts.rs`) and `explain()` must look for exactly
  (`app/test/explain_test.dart`).
- Fuzzing of every decoder of what comes from outside (other devices, pasted links, files on
  the disk): long runs of damaged inputs and libFuzzer targets, `tools/fuzz.sh`; see
  `docs/fuzzing.md`.
- `tools/netlab`: network namespaces with a simulated public network, a local DHT bootstrap
  node and NATs of different kinds (port-preserving, random port per destination), to run
  the connection scenarios automatically: members connecting directly, by scheduled punch
  or through a member; joining with an invite; becoming friends with a link or a code and
  meeting again later; accepting a request after the requester went offline; removing a
  friend; a file shared in a room between two hard NATs through a member; direct messages
  written while one friend's computer sleeps and the other's crashes, each arriving once and
  in order; the network check; and moving an account (5e) on one local network over mDNS
  (`transfer-lan`), between two cone NATs over the DHT and a punch (`transfer-cone-cone`), each
  followed by the moved account talking in its group, and between a cone and a symmetric NAT,
  which must say the two could not reach each other and change nothing
  (`transfer-cone-symmetric`); a release that spreads from the publisher to a friend and on
  to that friend's friend on another platform, while a stranger gets nothing
  (`update-chain`), and a modified copy whose unknown key, broken signature, older build,
  oversized package and damaged chunks are refused (`update-refusals`).
- `tools/beta`: the release tooling's tests (`test_release.py`, and `test_ci_builds.py` for the
  fetch and check of the builds GitHub makes), the macOS updater's
  (`test-macos-updater.sh`: what it refuses, with bundles signed for the test), and on Linux
  the Flatpak's, each in a flatpak installation of its own: what flatpak takes from the
  loopback origin (`test-flatpak-origin.sh`) and where the data key is kept and how it moves
  there (`test-flatpak-keychain.sh`, in a session of its own).
- `app/test`: the app's unit and widget tests (about 160 files: every screen and widget,
  the state the screens read, formatting, the order of video codecs in a description...),
  run with `flutter test` on every system.
- Compatibility between versions: the judgment of level two pinned on a corpus and compared
  with the frozen level-one copy (5.1); `crates/fogata-core/tests/previous_release.rs`, the
  command line of the last release against this version (weekly on GitHub, and before every
  release: docs/releasing-beta.md); and `crates/fogata-core/tests/old_copy.rs`, copies from
  before a given feature (run by hand where such a build is at hand). The plan for later
  levels is under "Later levels" in 5.1.
- `app/integration_test` (run on Linux, macOS and Windows): the application driven on a
  desktop with a fresh data folder:
  every screen (`screens_test.dart`), and a whole session with another device
  (`with_friend_test.dart`, with `app/tool/peer.sh` or `peer.ps1` running the CLI as the
  other device:
  friend link, group, room, chat, a file downloaded and checked, the other leaving), the
  window buttons (`window_test.dart`), and a call between copies of the app, on one
  computer or on several (`call_test.dart`, two to six copies: after the friend links are
  carried over, the copies take turns through the room's chat; they connect through their
  media bridges, hear each other, see a mute, notice a change of microphone, and, as
  asked, show a camera and a shared screen, logging round trips, losses, freezes and video
  sizes), and direct messages between two copies (`direct_test.dart`: a friend link, writing
  both ways, pictures that come by themselves and files that are fetched, the timer for
  both, clearing and asking again, what is written while the other copy is closed and goes
  once it starts again, and a chat that can only be read once the two are no longer
  friends). On Linux it runs with virtual sound devices (a PipeWire null sink, named by
  `PULSE_SINK`, which also takes the application's own sounds, and sources fed by it), a
  made-up camera (`FOGATA_TEST_CAMERA=1`, see `app/third_party/flutter_webrtc/FOGATA.md`)
  and a virtual display (Xvfb), so it touches nothing of the computer's own
  microphone, speakers or screen; a copy entered with the microphone off must not record at
  all, except from "Fogata silence" while it shares a screen's sound. It refuses to run without a data folder of its own. A virtual machine on a computer
  with a firewall may need a rule letting the machine's network reach that computer.
- Real-world tests between devices on different networks.
