import 'dart:io';

import 'package:flutter/foundation.dart';

import 'report.dart' show noteAppError, noteAppLine;

// What the window writes to its console. A release build's standard output and error may end
// in the system's journal (on Linux, where a desktop starts applications in a scope of
// systemd), which keeps them on a disk with nothing taken out: as the core's standard error
// (crates/fogata-ffi/src/logs.rs), they carry no message, which may hold a name, a path or
// an address. Flutter prints an uncaught error with its message and stack, the engine one
// that reached the root zone, and the window and its libraries print with debugPrint: in a
// release build each error writes one line saying what kind it was and where, and goes to
// the day's log, scrubbed ([noteAppError]); what is printed goes to the log alone
// ([noteAppLine]). A debug build (a developer's run, the tests, which read Flutter's
// reports) prints as Flutter does.

/// Whether the console takes what the window says as it is: a debug build.
const rawConsole = kDebugMode;

/// The line standard error gets for an error of the window: its kind and where in the code
/// it happened, without what it said.
String errorLine(Object error, StackTrace? stack) =>
    'Fogata: ${error.runtimeType} at ${placeOf(stack) ?? 'an unknown place'}; '
    'what it said is in the log, without what is personal';

/// Where in the code [stack] starts: the file and line of its first frame
/// (`main.dart:12`), without the folders before it, or in a release build, whose frames name
/// nothing, the place in the build's code (`_kDartIsolateSnapshotInstructions+0x2c5b27`);
/// null when it says neither.
String? placeOf(StackTrace? stack) {
  if (stack == null) return null;
  final named = RegExp(r'^#\d+\s+.*\(([^()]*?):(\d+)(?::\d+)?\)$');
  final built = RegExp(
    r'\b(_kDartIsolateSnapshotInstructions\+0x[0-9a-fA-F]+)',
  );
  for (final line in '$stack'.split('\n')) {
    final frame = named.firstMatch(line.trim());
    if (frame != null) {
      final file = frame.group(1)!.split(RegExp(r'[/\\]')).last;
      return '$file:${frame.group(2)}';
    }
    final place = built.firstMatch(line);
    if (place != null) return place.group(1);
  }
  return null;
}

/// Notes the window's errors in the day's log of [dataDir], for problem reports, and in a
/// release build tells the console only [errorLine] of each, and nothing of what is
/// printed. [write] is where that line goes: standard error.
void keepConsoleClean(
  String dataDir, {
  void Function(String line)? write,
  bool raw = rawConsole,
}) {
  final say = write ?? stderr.writeln;
  final flutter = FlutterError.onError;
  FlutterError.onError = (details) {
    noteAppError(dataDir, details.exception, details.stack);
    if (raw) {
      flutter?.call(details);
    } else {
      say(errorLine(details.exception, details.stack));
    }
  };
  final platform = PlatformDispatcher.instance.onError;
  PlatformDispatcher.instance.onError = (error, stack) {
    noteAppError(dataDir, error, stack);
    if (raw) return platform?.call(error, stack) ?? false;
    say(errorLine(error, stack));
    // Handled: left to the engine, it is printed as it is.
    return true;
  };
  if (!raw) {
    debugPrint = (message, {wrapWidth}) {
      if (message != null) noteAppLine(dataDir, message);
    };
  }
}
