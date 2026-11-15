# tray_manager in Fogata

This is tray_manager 0.5.3 from pub.dev (MIT, see LICENSE), without its `example` folder,
its Chinese README and its Linux plugin, its README.md a short note pointing here.

The Linux plugin (`linux/CMakeLists.txt`, `linux/include/tray_manager/tray_manager_plugin.h`,
`linux/tray_manager_plugin.cc`) is left out on purpose: it draws its icon with
libayatana-appindicator, which Fogata does not link, and the application serves its tray
icon on Linux itself, as a StatusNotifierItem over D-Bus (`crates/fogata-tray`, started
from `app/lib/src/window.dart`). So this copy is a plugin for macOS and Windows only:
`pubspec.yaml` no longer names Linux, and nothing else is changed.

Reviewed against tray_manager 0.9.0 (2026-11-15): the fork stays on 0.5.3, the last version
of the plugin as it was.

- 0.9.0 needs Flutter 3.50 and Dart 3.14, past the Flutter Fogata builds with (3.47.8), so
  0.8.3 is the newest Fogata could take at all.
- From 0.6.0 on, tray_manager has no code of its own for any system: it is a Dart layer on
  nativeapi and nativeapi_flutter, with the 0.5 API kept only as a deprecated bridge
  (`package:tray_manager/legacy.dart`). nativeapi's native part, cnativeapi, is a C++
  library of windows, menus, dialogs, the clipboard, shortcuts and tray icons (some 260
  source files) that a build hook compiles on every system the application is built for,
  Linux included, where Fogata has no use for it, and five more packages come with it
  (code_assets, hooks, logging, native_toolchain_c, ffi). Taking it means far more native
  code in every package than the two small plugins it replaces, for what follows.
- Its fixes, none of which Fogata's use needs (an icon, a tooltip and a context menu on
  Windows and macOS, `lib/src/window.dart`; Linux is Fogata's own):
  - 0.8.1 and 0.8.3, Windows: an elevated application's tray icon answers clicks again,
    and keeps answering after switching icons. Fogata never runs elevated and has one
    icon.
  - 0.8.0: a hot restart no longer leaves the old icon behind or crashes (debug builds
    only, and in nativeapi's code, which 0.5.3 does not have); web builds; Linux's
    StatusNotifier id and title; Windows' `getBounds` in logical pixels (Fogata does not
    call it).
  - 0.8.2: Linux only. 0.9.0: the Flutter and Dart above, and nativeapi 0.6.
