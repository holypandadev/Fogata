import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'autostart.dart' show ownAppImage;
import 'l10n.dart';
import 'private_file.dart';
import 'window.dart';

/// The network part stopping by itself (internal errors that kept coming back), and the
/// application starting again in a new process to get it back.
///
/// The application starts itself again once, a few seconds after the network part stops.
/// If it stops again within [_autoAgainAfter] of that, it does not: the banner stays, with
/// a button to start it again by hand. The copy that restarts leaves a small file next to
/// the window's lock (see [readRestart]): the new copy waits for the old one to let go of
/// the data folder, and knows when the last automatic restart was.
class Stopping extends ChangeNotifier {
  Stopping(this.dataDir);

  final String dataDir;

  /// The network part stopped; nothing reaches this device until it starts again.
  bool stopped = false;

  /// Starting again, by itself in a moment or because the person asked.
  bool restarting = false;

  /// The new copy is being started; this one quits next.
  bool starting = false;

  /// Why the last try to start again failed, to show.
  String? failure;

  Timer? _soon;

  /// How long after an automatic restart another one is left to the person.
  static const _autoAgainAfter = Duration(minutes: 10);

  /// How long the banner shows before the application starts again by itself.
  static const _autoAfter = Duration(seconds: 3);

  /// Whether this copy starts again by itself when the network part stops.
  bool get restartsByItself {
    final last = _lastAutomaticMs;
    return last == null ||
        DateTime.now().millisecondsSinceEpoch - last >=
            _autoAgainAfter.inMilliseconds;
  }

  void onStopped() {
    if (stopped) return;
    stopped = true;
    if (restartsByItself) {
      restarting = true;
      _soon = Timer(_autoAfter, () => restart(automatic: true));
    }
    notifyListeners();
  }

  /// Starts a new copy of the application and quits this one.
  Future<void> restart({bool automatic = false}) async {
    _soon?.cancel();
    if (starting) return;
    restarting = true;
    starting = true;
    failure = null;
    notifyListeners();
    final now = DateTime.now().millisecondsSinceEpoch;
    try {
      await writePrivate(
        _marker(dataDir).path,
        utf8.encode(
          jsonEncode({
            'at': now,
            if ((automatic ? now : _lastAutomaticMs) case final int last)
              'automatic': last,
          }),
        ),
      );
      await _startAgain();
    } catch (e) {
      debugPrint('cannot start Fogata again: $e');
      try {
        await _marker(dataDir).delete();
      } catch (_) {}
      restarting = false;
      starting = false;
      failure = l10n.restartFailed;
      notifyListeners();
      return;
    }
    await quitApplication();
  }
}

/// The arguments the application was started with, to start it again with the same.
List<String> launchArguments = const [];

/// When the application last started again by itself, if this copy comes from such a
/// restart (or from one by hand after it).
int? _lastAutomaticMs;

/// Whether this copy was started by a restart less than a minute ago.
bool startedByRestart = false;

/// Whether this copy was started by an automatic restart: the person is told why the
/// window came back.
bool get startedByItself =>
    startedByRestart &&
    _lastAutomaticMs != null &&
    DateTime.now().millisecondsSinceEpoch - _lastAutomaticMs! < 60 * 1000;

File _marker(String dataDir) => File('$dataDir/window/restart');

/// Reads what the copy that started this one left, and removes it.
Future<void> readRestart(String dataDir) async {
  final file = _marker(dataDir);
  try {
    final data = jsonDecode(await file.readAsString()) as Map<String, dynamic>;
    final at = data['at'] as int;
    final age = DateTime.now().millisecondsSinceEpoch - at;
    if (age >= 0 && age < 60 * 1000) {
      startedByRestart = true;
      _lastAutomaticMs = data['automatic'] as int?;
    }
  } catch (_) {}
  try {
    await file.delete();
  } catch (_) {}
}

/// Starts another copy of this application, detached from this one, with the same
/// arguments and environment. On macOS Launch Services opens the bundle again as a new
/// instance (the runner's `fogata/restart` channel), as the Finder would, so the system
/// sees the application itself start, sandbox and permissions included; the `FOGATA_*`
/// environment of a test copy goes along.
Future<void> _startAgain() async {
  if (Platform.isMacOS) {
    await const MethodChannel('fogata/restart')
        .invokeMethod<void>('startAgain', {
          'arguments': launchArguments,
          'environment': {
            for (final MapEntry(:key, :value) in Platform.environment.entries)
              if (key.startsWith('FOGATA_')) key: value,
          },
        });
    return;
  }
  await Process.start(
    relaunched(Platform.environment),
    launchArguments,
    mode: ProcessStartMode.detached,
  );
}

/// The program that starts this application again. Inside an AppImage the executable lives
/// in a folder mounted only while it runs; the AppImage itself is what starts it again, as
/// for starting with the computer ([ownAppImage]).
@visibleForTesting
String relaunched(Map<String, String> environment, {String? executable}) {
  final self = executable ?? Platform.resolvedExecutable;
  return ownAppImage(environment, self) ?? self;
}
