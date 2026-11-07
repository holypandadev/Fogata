# Fogata desktop application

The Flutter interface of Fogata. The core is Rust (`../crates/fogata-core`), reached through
`../crates/fogata-ffi` with flutter_rust_bridge. See the main README for building and
testing, and `../docs/architecture.md` (section 5d) for how it fits together.

- `lib/main.dart`: start-up.
- `lib/src/app_state.dart`: what the screens show, kept up to date from the core's events.
- `lib/src/screens/`: welcome, shell (header and sidebar), groups, new group, friends,
  room, settings.
- `lib/src/theme.dart`, `icons.dart`, `widgets.dart`: the design's tokens, icons and parts.
- `assets/fonts/`: Inter (SIL Open Font License, `Inter-OFL.txt`), the interface font off
  macOS, which uses its own SF Pro; Geist Mono (SIL Open Font License, `OFL.txt`); the
  Phosphor icons, regular and bold (MIT, `Phosphor-LICENSE.txt`).
- `integration_test/`, `tool/peer.sh`: the application driven on a desktop.
