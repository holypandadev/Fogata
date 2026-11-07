# tray_manager in Fogata

This is tray_manager 0.5.3 from pub.dev (MIT, see LICENSE), without its `example` folder,
its Chinese README and its Linux plugin. On Linux the plugin draws its icon with
libayatana-appindicator, which Fogata does not link; the application serves its tray icon
there itself (crates/fogata-tray), so this copy is a plugin for macOS and Windows only.
`pubspec.yaml` no longer names Linux, and nothing else is changed.
