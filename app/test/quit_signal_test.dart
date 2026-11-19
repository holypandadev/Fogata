import 'dart:async';
import 'dart:io';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/window.dart';

void main() {
  /// [signals] sent a second apart, SIGTERM and SIGINT in turn, to [quitOnSignals] with [quit]: how many times it quit,
  /// and when it ended the process, within 20 s of fake time.
  ({int quits, List<Duration> ends}) signalled(
    int signals,
    Future<void> Function() quit,
  ) {
    var quits = 0;
    final ends = <Duration>[];
    fakeAsync((time) {
      final sent = StreamController<ProcessSignal>();
      quitOnSignals(
        sent.stream,
        quit: () {
          quits++;
          return quit();
        },
        end: () => ends.add(time.elapsed),
      );
      for (var i = 0; i < signals; i++) {
        sent.add(i.isEven ? ProcessSignal.sigterm : ProcessSignal.sigint);
        time.elapse(const Duration(seconds: 1));
      }
      time.elapse(const Duration(seconds: 20));
    });
    return (quits: quits, ends: ends);
  }

  test('a signal quits, and the process ends at the limit if still there', () {
    // Quitting ends the process by itself (the window destroyed); here it does not.
    final once = signalled(1, () async {});
    expect(once.quits, 1);
    expect(once.ends, [const Duration(seconds: 10)]);
  });

  test('signals that come while quitting quit no more, nor move the end', () {
    final more = signalled(
      3,
      () => Future<void>.delayed(const Duration(seconds: 2)),
    );
    expect(more.quits, 1);
    expect(more.ends, [const Duration(seconds: 10)]);
  });

  test('quitting that fails ends the process at once', () {
    final failed = signalled(1, () async => throw StateError('no core'));
    expect(failed.quits, 1);
    expect(failed.ends, [Duration.zero]);
  });

  test('quitting that never ends still ends the process at the limit', () {
    final hung = signalled(1, () => Completer<void>().future);
    expect(hung.quits, 1);
    expect(hung.ends, [const Duration(seconds: 10)]);
  });

  test('nothing quits without a signal', () {
    final none = signalled(0, () async {});
    expect(none.quits, 0);
    expect(none.ends, isEmpty);
  });
}
