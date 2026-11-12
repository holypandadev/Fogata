import 'dart:io';

import 'package:flutter/services.dart';

import 'l10n.dart';
import 'test_switches.dart';

/// Whether Fogata opens when the person signs in to the computer, as the system has it.
enum AutostartState {
  on,
  off,

  /// Asked for, but macOS waits for the person to allow it in System Settings.
  needsApproval,

  /// This system, or this copy of Fogata, cannot do it.
  unsupported,
}

/// Opening Fogata when the person signs in: an XDG autostart entry on Linux, a login item
/// on macOS 13 and later, and a value under the user's Run key on Windows. The system
/// keeps the setting, so it is read from there rather than remembered, and someone who
/// takes it out in the system's own settings sees it off here too.
abstract class Autostart {
  Future<AutostartState> read();

  /// Turns it on or off, and says how it ended.
  Future<AutostartState> write(bool on);

  static Autostart forThisSystem() {
    if (Platform.isLinux) return LinuxAutostart();
    if (Platform.isMacOS || Platform.isWindows) return const RunnerAutostart();
    return const _Unsupported();
  }
}

/// What the settings use. Widget tests never reach the system's: they get one that
/// keeps it in memory, unless they put their own.
Autostart autostart = testSwitch('FLUTTER_TEST') != null
    ? MemoryAutostart()
    : Autostart.forThisSystem();

/// Kept in memory only, for tests.
class MemoryAutostart implements Autostart {
  MemoryAutostart([this.state = AutostartState.off]);
  AutostartState state;

  /// Every change asked for, in order.
  final writes = <bool>[];

  @override
  Future<AutostartState> read() async => state;

  @override
  Future<AutostartState> write(bool on) async {
    writes.add(on);
    if (state == AutostartState.unsupported) return state;
    return state = on ? AutostartState.on : AutostartState.off;
  }
}

class _Unsupported implements Autostart {
  const _Unsupported();

  @override
  Future<AutostartState> read() async => AutostartState.unsupported;

  @override
  Future<AutostartState> write(bool on) async => AutostartState.unsupported;
}

/// macOS (SMAppService.mainApp, in MainFlutterWindow.swift) and Windows (the Run key, in
/// flutter_window.cpp): the runner does it.
class RunnerAutostart implements Autostart {
  const RunnerAutostart();

  static const _channel = MethodChannel('fogata/autostart');

  static AutostartState _state(String? name) => switch (name) {
    'on' => AutostartState.on,
    'off' => AutostartState.off,
    'approval' => AutostartState.needsApproval,
    _ => AutostartState.unsupported,
  };

  @override
  Future<AutostartState> read() async {
    try {
      return _state(await _channel.invokeMethod<String>('get'));
    } on MissingPluginException {
      return AutostartState.unsupported;
    }
  }

  @override
  Future<AutostartState> write(bool on) async =>
      _state(await _channel.invokeMethod<String>('set', on));
}

/// Asks the Background portal (in the Linux runner) to start the application with the
/// session, or not: true when it will, false when it will not, null when the portal could
/// not be asked.
typedef RequestBackground = Future<bool?> Function(
  bool autostart,
  List<String> commandline,
);

Future<bool?> _requestBackground(
  bool autostart,
  List<String> commandline,
) async {
  try {
    return await const MethodChannel('fogata/desktop')
        .invokeMethod<bool>('requestBackground', {
          'autostart': autostart,
          'commandline': commandline,
          'reason': l10n.autostartReason,
        });
  } on MissingPluginException {
    return null;
  } on PlatformException {
    return null;
  }
}

/// The AppImage this copy runs from, when it runs from one: `$APPIMAGE`, but only while
/// [executable] is inside the folder the AppImage is mounted on (`$APPDIR`). A program
/// started from another AppImage (a launcher, a terminal) inherits both, and would
/// otherwise start that AppImage instead of itself.
String? ownAppImage(Map<String, String> environment, String executable) {
  final image = environment['APPIMAGE'];
  final mounted = environment['APPDIR'];
  if (image == null ||
      mounted == null ||
      !image.startsWith('/') ||
      !mounted.startsWith('/') ||
      mounted == '/') {
    return null;
  }
  final inside = mounted.endsWith('/') ? mounted : '$mounted/';
  return executable.startsWith(inside) ? image : null;
}

/// An entry in `$XDG_CONFIG_HOME/autostart` (`~/.config/autostart` by default) that
/// starts this executable; turned off, the entry goes. Inside a Flatpak, where that folder
/// is the sandbox's own and the desktop never reads it, the Background portal makes the
/// entry instead, and the sandbox's own entry only remembers what the portal answered.
class LinuxAutostart implements Autostart {
  LinuxAutostart({
    Map<String, String>? environment,
    this._executable,
    RequestBackground? background,
  }) : _environment = environment ?? Platform.environment,
       _background = background ?? _requestBackground;

  final Map<String, String> _environment;
  final String? _executable;
  final RequestBackground _background;

  /// The entry's name, after the application's id.
  static const fileName = 'dev.holypanda.fogata.desktop';

  /// The folder of autostart entries; null without a home to find it from.
  String? get folder {
    final config = _environment['XDG_CONFIG_HOME'];
    // The specification ignores a relative path.
    if (config != null && config.startsWith('/')) return '$config/autostart';
    final home = _environment['HOME'];
    if (home == null || home.isEmpty) return null;
    return '$home/.config/autostart';
  }

  File? get _file => folder == null ? null : File('$folder/$fileName');

  /// The program to start. Inside an AppImage the executable lives in a folder mounted
  /// only while it runs; the AppImage itself is what starts it again ([ownAppImage]).
  String get executable =>
      _executable ??
      ownAppImage(_environment, Platform.resolvedExecutable) ??
      Platform.resolvedExecutable;

  /// In a Flatpak the folder is the sandbox's own, which the desktop never reads.
  bool get _sandboxed => _environment.containsKey('FLATPAK_ID');

  @override
  Future<AutostartState> read() async {
    final file = _file;
    if (file == null) return AutostartState.unsupported;
    if (_sandboxed) {
      return await file.exists() ? AutostartState.on : AutostartState.off;
    }
    try {
      final text = await file.readAsString();
      // An entry hidden or turned off by the desktop's own settings does not start.
      final lines = text.split('\n').map((l) => l.trim());
      if (lines.contains('Hidden=true') ||
          lines.contains('X-GNOME-Autostart-enabled=false')) {
        return AutostartState.off;
      }
      return AutostartState.on;
    } on FileSystemException {
      return AutostartState.off;
    }
  }

  @override
  Future<AutostartState> write(bool on) async {
    final file = _file;
    if (file == null) return AutostartState.unsupported;
    if (_sandboxed) {
      final will = await _background(on, [executable]);
      if (will == null) return AutostartState.unsupported;
      if (will) {
        await file.parent.create(recursive: true);
        await file.writeAsString(
          entry(executable, namedIcon: iconInstalled),
          flush: true,
        );
        return AutostartState.on;
      }
      if (await file.exists()) await file.delete();
      return AutostartState.off;
    }
    if (!on) {
      if (await file.exists()) await file.delete();
      return AutostartState.off;
    }
    await file.parent.create(recursive: true);
    // Written beside it and moved over it, so the desktop never reads half an entry.
    final part = File('${file.path}.part');
    await part.writeAsString(
      entry(executable, namedIcon: iconInstalled),
      flush: true,
    );
    await part.rename(file.path);
    return AutostartState.on;
  }

  /// The application's icon in the desktop's icon theme, where an install put it.
  static const iconName = 'dev.holypanda.fogata';

  /// Whether an icon named [iconName] is installed where the desktop looks for icons: the
  /// hicolor theme or the pixmaps of `$XDG_DATA_HOME` and `$XDG_DATA_DIRS`.
  bool get iconInstalled {
    final home = _environment['XDG_DATA_HOME'];
    final user = _environment['HOME'];
    final system = _environment['XDG_DATA_DIRS'];
    final folders = [
      if (home != null && home.startsWith('/'))
        home
      else if (user != null && user.isNotEmpty)
        '$user/.local/share',
      ...(system == null || system.isEmpty
              ? '/usr/local/share:/usr/share'
              : system)
          .split(':')
          .where((f) => f.startsWith('/')),
    ];
    bool found(String base) =>
        File('$base.png').existsSync() || File('$base.svg').existsSync();
    for (final folder in folders) {
      if (found('$folder/pixmaps/$iconName')) return true;
      final theme = Directory('$folder/icons/hicolor');
      try {
        for (final size in theme.listSync().whereType<Directory>()) {
          if (found('${size.path}/apps/$iconName')) return true;
        }
      } on FileSystemException {
        continue;
      }
    }
    return false;
  }

  /// The entry that starts [executable]. Its icon is [iconName] with [namedIcon] (the icon
  /// is installed), otherwise the window's icon from the bundle, by its path, which moving
  /// the bundle leaves behind.
  static String entry(String executable, {bool namedIcon = false}) {
    final icon = '${File(executable).parent.path}/data/app_icon.png';
    return [
      '[Desktop Entry]',
      'Type=Application',
      'Name=Fogata',
      'Comment=Voice, video and text rooms for groups of friends',
      'Exec=${execArgument(executable)}',
      if (namedIcon)
        'Icon=$iconName'
      else if (File(icon).existsSync())
        'Icon=${_escapeValue(icon)}',
      'Terminal=false',
      'X-GNOME-Autostart-enabled=true',
      '',
    ].join('\n');
  }

  /// [path] as one argument of an Exec line: in double quotes, with the characters the
  /// Desktop Entry specification reserves inside them escaped, then escaped again as any
  /// string value is, and a percent sign doubled so it is not read as a field code.
  static String execArgument(String path) {
    final quoted = path.replaceAllMapped(
      RegExp(r'["`$\\]'),
      (m) => '\\${m[0]}',
    );
    return _escapeValue('"$quoted"').replaceAll('%', '%%');
  }

  /// A string value of a desktop entry: backslashes and line breaks escaped.
  static String _escapeValue(String value) => value
      .replaceAll(r'\', r'\\')
      .replaceAll('\n', r'\n')
      .replaceAll('\r', r'\r')
      .replaceAll('\t', r'\t');
}
