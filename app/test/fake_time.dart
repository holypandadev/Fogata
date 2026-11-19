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

/// [inFakeTime] with the calls' clock on that time ([callClock]); [stop] stops the calls
/// made in it once [body] ended, before the timers left are looked at. A stop cancels a
/// participant's tries ([Call]'s `_watch`), its connection's time to come up (`_connect`),
/// the question of which call is theirs (`_askWhich`) and the wait for its candidates
/// (`_describe`); the call's other waits (`_retry`,
/// `_watchOffer`, `_watchUpdate`, `_offerTurn`, `_askAgain`, the timeout in
/// `_withCandidates`) are not, and a test that leaves one is told so here.
Future<void> inCallTime(
  Future<void> Function() body, {
  Future<void> Function()? stop,
  Duration within = const Duration(minutes: 1),
}) => inFakeTime((time) async {
  final real = callClock;
  addTearDown(() => callClock = real);
  final start = DateTime(2026, 11, 19, 9);
  callClock = () => start.add(time.elapsed);
  await body();
  await stop?.call();
}, within: within);
