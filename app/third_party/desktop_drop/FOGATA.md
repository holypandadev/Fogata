# desktop_drop in Fogata

This is desktop_drop 0.8.4 from pub.dev (Apache-2.0, see LICENSE), without its `example`
and `test` folders, its README.md a short note pointing here. It stays on 0.8.4, the last
version before 0.9 (see the end).

Changed, each place marked `Fogata:` in the code:

- `linux/desktop_drop_plugin.cc` takes only a list of files (`text/uri-list`) from a drag,
  no longer plain text (`STRING`): any application, a web page among them, could drop text
  such as `.ssh/id_ed25519` that the Dart part read as a path. A drag that offers no list
  of files is not reported as something that can be dropped, and a drop that brings no
  data is ignored instead of reading a null pointer.
- `linux/desktop_drop_plugin.cc` also says which target the data came as, and
  `lib/src/channel.dart` (`linuxDropPaths`) takes only a `text/uri-list`, and of it only
  the `file:` entries of this computer (no host, or `localhost`, no query or fragment); a
  web address, a share (`smb:`), a comment or any other line is left out instead of being
  handed on as a path, and so is any other target (`text/x-moz-url`, `_NETSCAPE_URL`,
  `x-special/gnome-copied-files`, plain text). Its paths are read as POSIX ones on every
  system, as a Linux desktop writes them, so its tests give the same answer on Windows.
- `linux/desktop_drop_plugin.cc` hands its message's list and values over with
  `g_autoptr` and `fl_value_append_take`, so they are freed once sent: as published the list
  was never freed (0.8.4 frees the list, not the values in it).
- 0.8.1's FileTransfer portal (`application/vnd.portal.filetransfer`, through the dbus
  package, MPL-2.0) is left out: `lib/src/channel.dart` has no `performOperation_portal`,
  `pubspec.yaml` no dbus, and the plugin takes the portal's target from no drag. GTK 3 adds
  the portal's targets to the list of files' own where a portal runs, on any desktop; a drag
  that offers only those is shown as something that can be dropped, and its drop, which
  is not a `text/uri-list`, gives nothing, as in this copy of 0.8.0. A file dropped through
  the portal arrives as a path of the document portal (`/run/user/<id>/doc/<id>/<name>`), whatever
  the place it was dragged from: the application's check of private places
  (`app/lib/src/private_places.dart`) would let through a file dragged from `~/.ssh` or a
  browser's profile, as GTK hands every file of a drag to the portal by itself. Taking the
  portal needs that check to judge each file by its real place first (the Documents
  portal's `GetHostPaths`), inside the Flatpak only, and then tried on a real desktop.
- `lib/src/channel.dart`'s `performOperation_linux` is Fogata's (above); it hands on the
  text of a Linux drop as `rawText`, as published (the application does not read it), and
  `lib/src/events.dart` and `lib/src/drop_target.dart` say so of `rawText`, which here is
  never a portal's transfer key.
- `pubspec.yaml` no longer says `resolution: workspace`, which only fits the plugin's own
  repository.

Nothing else is changed: macOS, Windows, Android and the web are as published.

From 0.8.0 to 0.8.4, taken as published: `rawText` on `DropDoneEvent` and
`DropDoneDetails`, `lib/src/events.dart` exported, Android's Gradle set-up for both kinds of
Kotlin set-up (not built by Fogata).

Reviewed against desktop_drop 0.9.2 (2026-11-15): the fork stays on 0.8.4.

- 0.9.0 needs cross_file 0.4, and file_selector, which the application uses for its file
  picker, takes cross_file 0.3 only (file_selector_platform_interface 2.7.0, the newest):
  pub cannot give the application both. 0.9 can come once file_selector takes 0.4.
- 0.9.0 is also a rewrite: a native bridge through Dart's C API on every system, a new API
  (`onDrop` with typed items in place of `onDragDone`, `onEnter`/`onLeave`, `enabled`, one
  target per drop chosen by hit testing, temporary files of virtual sources that last
  only as long as `onDrop`), Flutter 3.47.7 and three new packages
  (cross_file_platform_interface, cross_file_web, mime). Taking it means every drop target
  of the application (`lib/src/outgoing.dart`) and the tests that drop through the plugin's
  channel written again. Its Linux code takes files only from a `text/uri-list`, as
  `file:` entries of this computer (or the portal's), which is what Fogata's change above
  does; a query in an entry is read there as part of the name.
- Its fixes: the portal transfer kept alive until its files are found (Linux; the portal
  is left out here, above), mouse events let through to native views outside a drag (macOS;
  Fogata embeds no native view), drops on the web. 0.9.1 (a crash at start on Windows,
  from tasks of 0.9.0's own native bridge) and 0.9.2 (Android's Gradle) fix 0.9.0's code
  only.
