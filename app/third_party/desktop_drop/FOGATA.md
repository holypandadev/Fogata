# desktop_drop in Fogata

This is desktop_drop 0.8.0 from pub.dev (Apache-2.0, see LICENSE), without its `example`
and `test` folders, its README.md a short note pointing here. Later versions depend on dbus (MPL-2.0), so this one stays.

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
