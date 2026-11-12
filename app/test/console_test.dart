import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/console.dart';
import 'package:fogata/src/report.dart';

import 'private_writer.dart';

void main() {
  setUpAll(writePrivatelyInTests);

  const said =
      'FileSystemException: Cannot open file, path = '
      '\'/home/marta/Pictures/beach.jpg\' (dial 203.0.113.7:4001)';
  const personal = ['marta', 'beach', '203.0.113.7', '4001', 'Cannot open'];

  test('an error tells the console only its kind and where it happened', () {
    final named = StackTrace.fromString(
      '#0      open (file:///home/marta/src/fogata/lib/src/files.dart:120:7)\n'
      '#1      main (package:fogata/main.dart:12:3)',
    );
    expect(placeOf(named), 'files.dart:120');
    // A release build's frames name nothing: the place in the build's code.
    final built = StackTrace.fromString(
      '*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** ***\n'
      'pid: 4321, tid: 4330, name io.flutter.ui\n'
      'build_id: \'8d1c0a4c2b9e\'\n'
      '#00 abs 00007f3a1b2c3d4e virt 0000000000391b27 '
      '_kDartIsolateSnapshotInstructions+0x2c5b27',
    );
    expect(placeOf(built), '_kDartIsolateSnapshotInstructions+0x2c5b27');
    expect(placeOf(null), isNull);
    final line = errorLine(const FileSystemException(said), named);
    expect(line, contains('FileSystemException at files.dart:120'));
    for (final gone in personal) {
      expect(line, isNot(contains(gone)), reason: line);
    }
  });

  test(
    'a release build tells the console nothing of what the window says',
    () async {
      final dir = Directory.systemTemp.createTempSync('fogata-console-');
      final (flutter, platform, printing) = (
        FlutterError.onError,
        PlatformDispatcher.instance.onError,
        debugPrint,
      );
      addTearDown(() {
        FlutterError.onError = flutter;
        PlatformDispatcher.instance.onError = platform;
        debugPrint = printing;
        forgetConsoleLog();
        dir.deleteSync(recursive: true);
      });
      final console = <String>[];
      // From the start, before the data folder is known: what comes then waits for it.
      guardConsole(write: console.add, raw: false);
      final stack = StackTrace.fromString(
        '#0 open (package:fogata/src/files.dart:120:7)',
      );
      FlutterError.onError!(
        FlutterErrorDetails(
          exception: const FileSystemException(said),
          stack: stack,
        ),
      );
      expect(console, hasLength(1));
      logConsoleIn(dir.path);
      expect(
        PlatformDispatcher.instance.onError!(StateError(said), stack),
        isTrue,
        reason: 'left to the engine, it is printed as it is',
      );
      debugPrint('microphone test: $said');
      await flushAppErrors();
      expect(console, hasLength(2));
      for (final line in console) {
        expect(line, contains('files.dart:120'));
        for (final gone in personal) {
          expect(line, isNot(contains(gone)), reason: line);
        }
      }
      // Problem reports still have what happened, scrubbed.
      final log = Directory('${dir.path}/$logFolder')
          .listSync()
          .whereType<File>()
          .single
          .readAsStringSync();
      expect(log, contains('ERROR app: FileSystemException'));
      expect(log, contains('ERROR app: Bad state'));
      expect(log, contains('INFO app: microphone test'));
      expect(log, contains('files.dart:120:7'));
      for (final gone in ['marta', 'beach', '203.0.113.7', '4001']) {
        expect(log, isNot(contains(gone)), reason: log);
      }
    },
  );

  test(
    'what the start says before the data folder is known waits, bounded',
    () async {
      final dir = Directory.systemTemp.createTempSync('fogata-console-start-');
      final (flutter, platform, printing) = (
        FlutterError.onError,
        PlatformDispatcher.instance.onError,
        debugPrint,
      );
      addTearDown(() {
        FlutterError.onError = flutter;
        PlatformDispatcher.instance.onError = platform;
        debugPrint = printing;
        forgetConsoleLog();
        dir.deleteSync(recursive: true);
      });
      guardConsole(
        write: (_) => fail('nothing printed reaches the console'),
        raw: false,
      );
      for (var i = 0; i < 100; i++) {
        debugPrint('the window opened at ‹$i›');
      }
      logConsoleIn(dir.path);
      await flushAppErrors();
      final log = Directory('${dir.path}/$logFolder')
          .listSync()
          .whereType<File>()
          .single
          .readAsStringSync();
      expect('INFO app: the window opened'.allMatches(log), hasLength(64));
      expect(
        log,
        contains('36 more errors and lines of the start were not kept'),
      );
    },
  );
}
