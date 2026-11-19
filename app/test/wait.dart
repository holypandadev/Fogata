import 'package:flutter_test/flutter_test.dart';

/// The longest a test waits for real work (a file read, a picture decoded, the system's
/// file watch) before it says what never came. It is a safety net, not a timing: the
/// waits below end as soon as what they wait for has happened, so a busy computer only
/// makes them take longer, and a real hang still fails, with its reason, before the
/// test's own time out.
const realWorkLimit = Duration(seconds: 20);

/// Pumps frames, letting real work go on between them, until [done] holds; fails saying
/// [what] when it does not within [realWorkLimit].
Future<void> pumpUntil(
  WidgetTester tester,
  bool Function() done, {
  required String what,
}) async {
  final waited = Stopwatch()..start();
  while (!done()) {
    if (waited.elapsed > realWorkLimit) {
      fail('waited ${realWorkLimit.inSeconds} s for $what');
    }
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump();
  }
}

/// Waits, outside a widget test, until [done] holds; fails saying [what] when it does not
/// within [limit]. The limit is counted on the time the test runs in: the real one, or
/// fake time ([inFakeTime]), which moves only by the waits here.
Future<void> until(
  bool Function() done, {
  required String what,
  Duration limit = realWorkLimit,
}) async {
  final real = Stopwatch()..start();
  const step = Duration(milliseconds: 10);
  var waited = Duration.zero;
  while (!done()) {
    if (real.elapsed > limit || waited > limit) {
      fail('waited ${limit.inSeconds} s for $what');
    }
    await Future<void>.delayed(step);
    waited += step;
  }
}
