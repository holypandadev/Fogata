import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';

/// Runs [body] in fake time: its timers and waits of seconds take none. [body] is given
/// that time, to put in place of the clocks of what it tests; it must end within [within]
/// of it.
void inFakeTime(
  Future<void> Function(FakeAsync time) body, {
  Duration within = const Duration(minutes: 1),
}) {
  fakeAsync((time) {
    var done = false;
    Object? error;
    StackTrace? trace;
    body(time).then<void>(
      (_) => done = true,
      onError: (Object e, StackTrace t) => (error, trace, done) = (e, t, true),
    );
    time.elapse(within);
    if (error != null) Error.throwWithStackTrace(error!, trace!);
    expect(done, isTrue, reason: 'it ended within $within of fake time');
  });
}
