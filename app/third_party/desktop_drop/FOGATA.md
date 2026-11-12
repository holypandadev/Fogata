# desktop_drop in Fogata

This is desktop_drop 0.8.0 from pub.dev (Apache-2.0, see LICENSE), without its `example`
and `test` folders, its README.md a short note pointing here. It stays on 0.8.0 (see the
end).

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
- `pubspec.yaml` no longer says `resolution: workspace`, which only fits the plugin's own
  repository.

Nothing else is changed: macOS, Windows, Android and the web are as published.

Reviewed against desktop_drop 0.9.2 (2026-11-12): the fork stays on 0.8.0.

- 0.8.1 to 0.8.4 depend on the dbus package (MPL-2.0), for 0.8.1's one feature: on Linux, a
  drag from a sandboxed application hands its files through the FileTransfer portal
  (`application/vnd.portal.filetransfer`), whose key the plugin turns into paths the
  document portal opens. The rest is Android's Gradle set-up.
- 0.9.0 is a rewrite: a native bridge through Dart's C API on every system, a new API
  (`onDrop` with typed items in place of `onDragDone`, `onEnter`/`onLeave`, `enabled`, one
  target per drop chosen by hit testing), Flutter 3.47.7, cross_file 0.4 and three new
  packages (cross_file_platform_interface, cross_file_web, mime). Its Linux code is new,
  so Fogata's change above would have to be made again on it, and every drop target of the
  application rewritten (`lib/src/outgoing.dart`). Its fixes: the portal transfer kept
  alive until its files are found (Linux, the 0.8.1 feature), mouse events let through to
  native views outside a drag (macOS; Fogata embeds no native view), drops on the web.
- 0.9.1 (a crash at start on Windows, from tasks of 0.9.0's own native bridge) and 0.9.2
  (Android's Gradle) fix 0.9.0's code only.

Nothing there fixes what Fogata uses of 0.8.0 (files dropped on a window, on Linux, Windows
and macOS). One thing is worth a decision: in the Flatpak, which sees no more of the
computer than its downloads folder, a file dragged from outside it arrives, by the code (not
tried), as a path the application cannot read, unless the source hands it over through the FileTransfer portal,
which 0.8.0 does not take. Taking that portal's target on Linux, with the portal asked
through GIO (no new package), in Fogata's `linux/desktop_drop_plugin.cc`, would let such
drops through.
