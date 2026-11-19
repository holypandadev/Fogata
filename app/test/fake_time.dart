import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

/// Runs [body] in fake time: its timers and waits of seconds take none. [body] is given
/// that time, to put in place of the clocks of what it tests; it must end within [within]
/// of it, and leave no timer behind (one would fire in a later test), but those made by
/// the functions [timersLeftBy] names, each with why it is left.
///
/// What resumes on the real event loop rather than in the zone of the fake time (the
/// cancel of a broadcast stream's subscription completes there) goes on between the fake
/// milliseconds.
Future<void> inFakeTime(
  Future<void> Function(FakeAsync time) body, {
  Duration within = const Duration(minutes: 1),
  Map<String, String> timersLeftBy = const {},
}) async {
  final time = FakeAsync();
  var done = false;
  Object? error;
  StackTrace? trace;
  time.run(
    (time) => body(time).then<void>(
      (_) => done = true,
      onError: (Object e, StackTrace t) => (error, trace, done) = (e, t, true),
    ),
  );
  // A millisecond at a time, so that what is left once it ended is not run.
  while (!done && time.elapsed < within) {
    time.elapse(const Duration(milliseconds: 1));
    await Future<void>.value();
  }
  if (error != null) Error.throwWithStackTrace(error!, trace!);
  expect(done, isTrue, reason: 'it ended within $within of fake time');
  final left = [
    for (final timer in time.pendingTimers)
      if (!timersLeftBy.keys.any(timer.debugString.contains)) timer.debugString,
  ];
  expect(
    left,
    isEmpty,
    reason: '${left.length} timers left pending when it ended',
  );
}

/// The waits of a call that its [Call.stop] does not end, which outlive it: each one,
/// once due, finds the call stopped and does nothing more.
const callWaitsAfterStop = {
  // `Future.delayed`, which cannot be cancelled.
  'Call._connect': 'the time a new connection has to come up (15 s)',
  '_Signalling._watch': 'the next round of tries for someone (5 s and more)',
  // A `Future.timeout` on the candidates of a description being made when it stopped.
  '_Signalling._describe': 'the wait for the candidates of a description (3 s)',
};

/// [inFakeTime] with the calls' clock on that time ([callClock]); [stop] stops the calls
/// made in it once [body] ended, before the timers left are looked at, of which only
/// [callWaitsAfterStop] may be.
Future<void> inCallTime(
  Future<void> Function() body, {
  Future<void> Function()? stop,
}) => inFakeTime((time) async {
  final real = callClock;
  addTearDown(() => callClock = real);
  final start = DateTime(2026, 11, 19, 9);
  callClock = () => start.add(time.elapsed);
  await body();
  await stop?.call();
}, timersLeftBy: callWaitsAfterStop);
