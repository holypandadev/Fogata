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
        lastResort: LastResort(end: () => ends.add(time.elapsed), note: (_) {}),
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

  /// A [LastResort] armed at each of [arms] and told to end now at each of [nows], within
  /// 20 s of fake time: when it ended the process.
  List<Duration> lastResort({
    List<Duration> arms = const [],
    List<Duration> nows = const [],
  }) {
    final ends = <Duration>[];
    fakeAsync((time) {
      final limit = LastResort(end: () => ends.add(time.elapsed), note: (_) {});
      for (final at in arms) {
        Timer(at, limit.arm);
      }
      for (final at in nows) {
        Timer(at, limit.now);
      }
      time.elapse(const Duration(seconds: 20));
    });
    return ends;
  }

  test(
    'the last resort ends the process once, at the limit from the first quit',
    () {
      expect(lastResort(arms: [Duration.zero]), [const Duration(seconds: 10)]);
      expect(lastResort(arms: [Duration.zero, const Duration(seconds: 3)]), [
        const Duration(seconds: 10),
      ]);
    },
  );

  test(
    'the last resort told to end now ends once, and not again at the limit',
    () {
      expect(
        lastResort(arms: [Duration.zero], nows: [const Duration(seconds: 2)]),
        [const Duration(seconds: 2)],
      );
      expect(
        lastResort(
          nows: [const Duration(seconds: 1)],
          arms: [const Duration(seconds: 2)],
        ),
        [const Duration(seconds: 1)],
      );
    },
  );

  test('the last resort never armed ends nothing', () {
    expect(lastResort(), isEmpty);
  });

  test(
    'a signal whose quit begins quitting again is ended once, at the limit',
    () {
      final ends = <Duration>[];
      fakeAsync((time) {
        final sent = StreamController<ProcessSignal>();
        final limit = LastResort(
          end: () => ends.add(time.elapsed),
          note: (_) {},
        );
        // As the tray's Quit does, which a signal goes through.
        quitOnSignals(
          sent.stream,
          quit: () async => limit.arm(),
          lastResort: limit,
        );
        sent.add(ProcessSignal.sigterm);
        time.elapse(const Duration(seconds: 20));
      });
      expect(ends, [const Duration(seconds: 10)]);
    },
  );

  /// A [LastResort] armed at the start, its process gone (nothing runs any more) at
  /// [goneAt] if given, and told to end now at [nowAt] if given, within 20 s of fake time:
  /// the lines it wrote and when it ended the process.
  ({List<String> lines, List<Duration> ends}) noted({
    Duration? goneAt,
    Duration? nowAt,
  }) {
    final lines = <String>[];
    final ends = <Duration>[];
    fakeAsync((time) {
      final limit = LastResort(
        end: () => ends.add(time.elapsed),
        note: lines.add,
      );
      limit.arm();
      if (nowAt != null) Timer(nowAt, limit.now);
      // A process that ended runs nothing more: the clock stops for it.
      time.elapse(goneAt ?? const Duration(seconds: 20));
    });
    return (lines: lines, ends: ends);
  }

  test('the last resort writes one line of why before it ends the process', () {
    final late = noted();
    expect(late.ends, [const Duration(seconds: 10)]);
    expect(late.lines, [
      'quitting was still running 10 s after it began and the process was ended '
          '(leaving finished: no, tray taken away: no, window destruction asked: no, '
          'exit request answered: no)',
    ]);
    final failed = noted(nowAt: const Duration(seconds: 1));
    expect(failed.ends, [const Duration(seconds: 1)]);
    expect(failed.lines, hasLength(1));
    expect(
      failed.lines.single,
      startsWith('quitting failed and the process was ended'),
    );
  });

  test('a quit that ends the process in time writes nothing', () {
    final inTime = noted(goneAt: const Duration(milliseconds: 9999));
    expect(inTime.lines, isEmpty);
    expect(inTime.ends, isEmpty);
  });

  test('a line that cannot be written still ends the process', () {
    final ends = <Duration>[];
    fakeAsync((time) {
      LastResort(
        end: () => ends.add(time.elapsed),
        note: (_) => throw const FileSystemException('full'),
      ).arm();
      time.elapse(const Duration(seconds: 20));
    });
    expect(ends, [const Duration(seconds: 10)]);
  });
}
