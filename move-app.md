# Moving a full group: what the app needs

Branch with the Rust side: lab/move-lab at 074529f (on lab/level-five-corpus). Flutter and the bridge code generator are not on the machine that wrote it, so the FFI and the app are left for a machine that has them.

## What core gives now

- `Core::move_group(group) -> GroupId`: moves a group to a new log as its first member. The new group takes its place here at once.
- `Core::moved_to(group) -> Option<GroupId>`: the group that shows in place of `group` once it moved and this device followed (the end of a line of moves). None otherwise.
- `GroupView::fill: Option<f64>`: how much of the log's ceiling the group holds, 0 to 1, from level five; None below it.
- `GroupView::may_move: bool`: this device is first on the ladder, the group judges at five or higher, and it has not moved.

## FFI (crates/fogata-ffi)

1. `UiGroup`: add `fill: Option<f64>` and `may_move: bool`, copied in `types.rs` `group()`.
2. `app.rs`: `move_group(group_id: String) -> Result<String>` (the new id, in hex) and `group_moved_to(group_id: String) -> Result<Option<String>>`.
3. Regenerate the bridge (flutter_rust_bridge_codegen 2.13.0).

## App

1. Fill warning: when `fill >= 0.75`, the group's settings say the group's history is nearly full. For the member with `may_move`, it says the group should move to a fresh history, with the button below. From `fill >= 1.0`, it says that only removals, leaves and the move still work until it moves.
2. Move button, in the group's settings, only when `may_move`: a confirm dialog saying that everyone keeps the group, its name and its history, the others follow when they connect, and members away for more than 30 days may have to be added again. On yes it calls `move_group`.
3. Following: on `GroupsChanged`, if the group open on screen is no longer listed, call `group_moved_to(id)`; when it gives an id, switch the open group, its room and any selection to it, without a message. Only when it gives None does the group leave the screen as it does today.
4. l10n for the new strings, in every language the app has.
5. Widget tests: the warning shows at 0.75 and 1.0 for the first member and not below; the button only with `may_move`; a group that moved stays open under its new id.

## After that

`Level::RAISED_TO` goes to five once the app part is merged and the network lab's group-move scenario passes.
