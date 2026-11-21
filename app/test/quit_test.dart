import 'dart:async';
import 'dart:io';
import 'dart:ui' show AppExitResponse;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/report.dart';
import 'package:fogata/src/window.dart';

import 'private_writer.dart';

void main() {
  setUpAll(writePrivatelyInTests);
  testWidgets(
    'the window\'s errors still waiting are on the disk as it quits',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => null,
      );
      final dir = Directory.systemTemp.createTempSync('fogata-quit-');
      addTearDown(() => dir.deleteSync(recursive: true));
      await tester.runAsync(() async {
        await beforeQuitting(
          () async {},
          keepRunning: () => false,
          lastResort: LastResort(end: () {}, note: (_) {}),
        );
        noteAppError(dir.path, 'the first error', null);
        await flushAppErrors();
        // Within a second of the first: it waits for that second to end.
        noteAppError(dir.path, 'the last error', null);
        expect(
          await tester.binding.handleRequestAppExit(),
          AppExitResponse.exit,
        );
        final log = Directory('${dir.path}/$logFolder')
            .listSync()
            .whereType<File>()
            .map((f) => f.readAsStringSync())
            .join();
        expect(log, contains('the first error'));
        expect(log, contains('the last error'));
      });
    },
  );

  /// Sets up quitting as the application does, with [before] for the shutdown, then runs
  /// [act] and 20 s of fake time: when the window was destroyed and when the last resort
  /// ended the process, from the start of [act]. With [destroyEnds], destroying the window
  /// ends the process, as it does when nothing stalls: nothing runs after it.
  Future<({List<Duration> destroyed, List<Duration> ends, List<String> lines})>
  quitting(
    WidgetTester tester, {
    required Future<void> Function() before,
    required void Function() act,
    bool destroyEnds = false,
  }) async {
    final start = tester.binding.clock.now();
    Duration since() => tester.binding.clock.now().difference(start);
    var gone = false;
    final destroyed = <Duration>[];
    final ends = <Duration>[];
    final lines = <String>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async {
        if (call.method == 'destroy' && !gone) {
          destroyed.add(since());
          gone = destroyEnds;
        }
        return null;
      },
    );
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('tray_manager'),
      (call) async => null,
    );
    await tester.runAsync(
      () => beforeQuitting(
        before,
        keepRunning: () => false,
        lastResort: LastResort(
          end: () {
            if (!gone) ends.add(since());
          },
          note: (line) {
            if (!gone) lines.add(line);
          },
        ),
      ),
    );
    act();
    for (var i = 0; i < 200; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    return (destroyed: destroyed, ends: ends, lines: lines);
  }

  testWidgets(
    'a quit from the menu that does not end the process is ended at the limit',
    (tester) async {
      final quit = await quitting(
        tester,
        before: () => Completer<void>().future,
        act: () => unawaited(quitApplication()),
      );
      // The shutdown had its whole 6 s before the window went.
      expect(quit.destroyed, hasLength(1));
      expect(
        quit.destroyed.single,
        greaterThanOrEqualTo(const Duration(seconds: 6)),
      );
      expect(quit.ends, [const Duration(seconds: 10)]);
      // What it writes says how far quitting had come: here, past everything it asks.
      expect(quit.lines, [
        'quitting was still running 10 s after it began and the process was ended '
            '(leaving finished: yes, tray taken away: yes, window destruction asked: yes, '
            'exit request answered: no)',
      ]);
    },
  );

  // With destroyEnds the fake end and note ignore every call once the window is destroyed,
  // so this test cannot tell the timer firing from its not firing: a real teardown, which
  // ends the process and every timer with it, cannot be simulated here. That the last
  // resort does nothing before its limit is told in quit_signal_test.dart.
  testWidgets('a quit that ends the process in time is not ended again', (
    tester,
  ) async {
    final quit = await quitting(
      tester,
      before: () => Completer<void>().future,
      act: () => unawaited(quitApplication()),
      destroyEnds: true,
    );
    expect(quit.destroyed, hasLength(1));
    expect(quit.ends, isEmpty);
    expect(quit.lines, isEmpty);
  });

  testWidgets(
    'quitting twice ends the process once, at the limit from the first',
    (tester) async {
      final quit = await quitting(
        tester,
        before: () => Future<void>.delayed(const Duration(seconds: 2)),
        act: () {
          unawaited(quitApplication());
          Timer(const Duration(seconds: 3), () => unawaited(quitApplication()));
        },
      );
      expect(quit.ends, [const Duration(seconds: 10)]);
    },
  );

  testWidgets('nothing is ended while the application is not quitting', (
    tester,
  ) async {
    final quit = await quitting(tester, before: () async {}, act: () {});
    expect(quit.destroyed, isEmpty);
    expect(quit.ends, isEmpty);
    expect(quit.lines, isEmpty);
  });

  test('a line written at once is in the day\'s log of the window', () {
    final dir = Directory.systemTemp.createTempSync('fogata-quit-now-');
    addTearDown(() => dir.deleteSync(recursive: true));
    noteAppErrorNow(dir.path, 'the last line');
    noteAppErrorNow(dir.path, 'and one more');
    final logs = Directory('${dir.path}/$logFolder')
        .listSync()
        .whereType<File>();
    expect(logs, hasLength(1));
    expect(logs.single.path, matches(RegExp(r'app-\d{4}-\d{2}-\d{2}\.log$')));
    final text = logs.single.readAsLinesSync();
    expect(text, hasLength(2));
    expect(text.first, endsWith(' ERROR app: the last line'));
    expect(text.last, endsWith(' ERROR app: and one more'));
  });
}
