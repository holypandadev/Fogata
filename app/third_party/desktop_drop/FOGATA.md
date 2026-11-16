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
- 0.8.1's FileTransfer portal (`application/vnd.portal.filetransfer`) is taken in
  `linux/desktop_drop_plugin.cc` itself, through GIO, not in the Dart part through the dbus
  package (MPL-2.0), which this copy does not depend on (`lib/src/channel.dart` has no
  `performOperation_portal`, `pubspec.yaml` no dbus), and inside the Flatpak only (where
  `/.flatpak-info` is). There the sandbox sees none of the computer's files but the
  downloads folder, so the list of files of a drag from a file manager names paths it
  cannot open. The portal's targets (`application/vnd.portal.filetransfer`, and the older
  `application/vnd.portal.files` GTK 3 also offers) come before the list of files: GTK takes
  the first target of the drop's list the source offers, and GTK and KDE applications offer
  the portal's beside the list.
- The portal gives this application every file of a transfer it retrieves (`RetrieveFiles`
  exports each one to it, to read, for good: the application can neither give a document
  back nor delete it), before anything is judged, and GTK hands every file a drag names to
  the portal by itself, a page's drag that names `~/.ssh/id_ed25519` among them. So inside
  the Flatpak the plugin takes every drop by hand (`drag-drop`, `gtk_drag_get_data`,
  `gtk_drag_finish`; not `GTK_DEST_DEFAULT_DROP`). For a drop that offers the portal's
  target it asks the Dart part first (`checkPortalDrop`) where the drop is and, when the
  source also offers the list of files, the list's text and the paths the plugin's GIO
  reads in it (the source's own GIO builds the transfer from the same list, and reads as
  files entries the Dart part does not: a query, a fragment, another host). The Dart part
  answers -2 when no drop target that takes drops holds the point (`DesktopDrop.holds`),
  -1 when the list cannot be judged (`portalListPaths`: every entry must be a file of this
  computer by `linuxDropPaths`, at least one, with one absolute path of GIO's for each) or
  there is no judge or it fails, else how many entries `DesktopDrop.portalDropCheck`, which
  the application sets (the rules stay there), refuses by either reading. Only 0 asks for
  the portal's target and the portal for the files. Otherwise the portal is never asked,
  nothing is exported, and the drop is handed on as `declined` with that number, or
  `unreached` for -1 (`portalRefusal` and `portalCount` on `DropDoneEvent` and
  `DropDoneDetails`, files empty), or not at all for -2. A source that offers no list is
  judged by where the drop is alone. Before the portal is asked the drop may take five
  seconds (a list that never comes, a Dart part that never answers: `unreached`); once it
  is asked (asynchronously), its answer is taken, or judged, whenever it comes within 20
  seconds, since a source may end its transfer once the drop is finished (0.9.0 also asks
  the portal in its plugin, before it finishes the drag). A portal that answers later has
  still exported the files, which are then not taken, and the person is told they could not
  be reached. The files are handed on as a `text/uri-list` of the paths the portal gave:
  the document portal's (`/run/user/<id>/doc/<id>/<name>`), or the file's own where the
  sandbox sees it already (the downloads folder). A key that is empty or not UTF-8, no
  session bus (got once, without holding up the window), a portal that gives nothing, is
  handed on as `unreached`, with the number of files the list named (1 when it was not
  read). One drop at a time: a new one ends the one before (without finishing it when it is
  the same drag context, as Wayland keeps one a seat), data is taken only when it is what
  the drop going asked for at the time of its own `drag-drop`, and a window destroyed
  meanwhile ends it. What is reported back to the source does not depend on whether the
  files could be taken (a page could learn from it whether a file it named is there): a
  drop that offered the portal's target is always reported taken. A drop of no target the
  plugin takes is finished as refused. A source that offers only the list of files is read
  as before. Outside the Flatpak the targets and the drop are those of 0.8.0: the list first,
  then the portal's, which GTK adds where a portal runs and which drop nothing, taken by
  `GTK_DEST_DEFAULT_ALL`.
- A document portal's path says nothing of where the file really is. So the plugin also
  answers `documentHostPaths` (`lib/src/channel.dart`), asking the Documents portal where
  the documents with the ids given really are (`GetHostPaths`, version 5 of its interface,
  which xdg-desktop-portal's changelog lists under 1.19.0; five seconds at most) and giving
  each path as its bytes; an id the portal does not answer for (not given to this
  application) is left out, and when the portal cannot be asked, is older or gives no
  answer in time, the call fails. The application asks once for a drop's files and judges
  each by that place (`app/lib/src/private_places.dart`, `realPlaces`): one the portal does
  not vouch for is not added, and one whose place cannot be learnt is not added either,
  with a notice of its own. This check stays after the one by names: a link in a plain
  folder to `~/.ssh/id_ed25519` passes the list's names, the portal follows it, and only
  its real place shows where it leads. That file is not added, but it stays exported to
  the application, which cannot give it back. A file whose name is not UTF-8 cannot be
  dropped inside the Flatpak: the portal's answer is read with such bytes replaced, and no
  longer ends in the document's name, so it is not taken.
- `lib/src/channel.dart`'s `performOperation_linux` is Fogata's (above); it hands on the
  text of a Linux drop as `rawText`, as published (the application does not read it), and
  `lib/src/events.dart` and `lib/src/drop_target.dart` say so of `rawText`, which here is
  never a portal's transfer key: the plugin hands on the portal's files as a list.
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
- Its fixes: the portal transfer kept alive until its files are found (Linux; done here in
  the plugin, above), mouse events let through to native views outside a drag (macOS;
  Fogata embeds no native view), drops on the web. 0.9.1 (a crash at start on Windows,
  from tasks of 0.9.0's own native bridge) and 0.9.2 (Android's Gradle) fix 0.9.0's code
  only.
