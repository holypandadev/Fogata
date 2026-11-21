import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/window.dart';

// Alone in its file: the binding asks every listener set up before it, one after another,
// and one set up in another test's clock would never answer here.
void main() {
  testWidgets('a quit through the exit request says so in its line', (
    tester,
  ) async {
    final start = tester.binding.clock.now();
    Duration since() => tester.binding.clock.now().difference(start);
    final destroyed = <Duration>[];
    final ends = <Duration>[];
    final lines = <String>[];
    for (final channel in ['window_manager', 'tray_manager']) {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        MethodChannel(channel),
        (call) async {
          if (call.method == 'destroy') destroyed.add(since());
          return null;
        },
      );
    }
    await tester.runAsync(
      () => beforeQuitting(
        () async {},
        keepRunning: () => false,
        lastResort: LastResort(end: () => ends.add(since()), note: lines.add),
      ),
    );
    unawaited(tester.binding.handleRequestAppExit());
    for (var i = 0; i < 200; i++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    // Answered with exit, the system ends the process: the tray and the window are left
    // to it, and here it does not.
    expect(destroyed, isEmpty);
    expect(ends, [const Duration(seconds: 10)]);
    expect(lines, [
      'quitting was still running 10 s after it began and the process was ended '
          '(leaving finished: yes, tray taken away: no, window destruction asked: no, '
          'exit request answered: yes)',
    ]);
  });
}
