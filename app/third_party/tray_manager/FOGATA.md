# tray_manager in Fogata

This is tray_manager 0.5.3 from pub.dev (MIT, see LICENSE), without its `example` folder,
its Chinese README and its Linux plugin, its README.md a short note pointing here. On Linux the plugin draws its icon with
libayatana-appindicator, which Fogata does not link; the application serves its tray icon
there itself (crates/fogata-tray), so this copy is a plugin for macOS and Windows only.
`pubspec.yaml` no longer names Linux, and nothing else is changed.

Reviewed against tray_manager 0.9.0 (2026-11-12): the fork stays on 0.5.3, the last version
of the plugin as it was. From 0.6.0 on, tray_manager is a rewrite on nativeapi and
nativeapi_flutter (one C++ core for every system, new packages Fogata would take in), with
the 0.5 API kept only as a deprecated bridge (`package:tray_manager/legacy.dart`), Flutter
3.47 (0.7.0) and then 3.50 with Dart 3.14 (0.9.0), past the Flutter Fogata builds with.
Its fixes, none of which Fogata's use needs (an icon, a tooltip and a context menu on
Windows and macOS, `lib/src/window.dart`; Linux is Fogata's own):

- 0.8.1 and 0.8.3, Windows: an elevated application's tray icon answers clicks again, and
  keeps answering after switching icons. Fogata never runs elevated and has one icon.
- 0.8.0: a hot restart no longer leaves the old icon behind or crashes (debug builds only;
  and in nativeapi's code, which 0.5.3 does not have); web builds; Linux's StatusNotifier
  id and title; Windows' `getBounds` in logical pixels (Fogata does not call it).
- 0.8.2: Linux only.
