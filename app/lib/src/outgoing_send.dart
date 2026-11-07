import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:fogata_video/fogata_video.dart';

import 'photo.dart' show isVideo;
import 'rust/api/types.dart';
import 'state/room_state.dart';

/// How many files go in one direct message: the core's `MAX_FILES`, which refuses more.
const maxOutgoingFiles = 10;

/// How many videos are read for their first picture at once, in the whole application
/// (every chat and room together): each takes a thread of the system's.
const videoProbesAtOnce = 2;

/// How long a video is waited for, its turn included, before it goes without its first
/// picture; changed by tests.
@visibleForTesting
Duration videoProbeTime = const Duration(seconds: 15);

/// How long a reading that never ends holds its turn: past it the turn goes back all the
/// same, as the next video asks for one, so two readings stuck for good (a file share that hangs, say) do not leave every
/// later video waiting [videoProbeTime] for nothing until Fogata restarts. The plugins
/// read two at most at once on their own, and answer a third at once, unread, while those
/// two still hold the system; changed by tests.
@visibleForTesting
Duration videoProbeHeldMost = const Duration(minutes: 1);

/// Reads a video's first picture and length; the tests put their own in its place.
@visibleForTesting
Future<VideoProbe?> Function(String path) videoProbe = probeVideo;

/// The turns to read a video, shared by every send of the application. A turn is held
/// until the system is done with that video, not just until the send stopped waiting for
/// it: a reading that outlives its time still holds a thread of the system's.
final _turns = _Turns(videoProbesAtOnce);

/// How many turns are taken and not given back; none once every reading has ended.
@visibleForTesting
int get videoTurnsHeld => _turns._held.length;

class _Turns {
  _Turns(this._free);
  int _free;
  final _waiting = <Completer<_Turn?>>[];

  /// Turns taken and not given back, oldest first.
  final _held = <_Turn>[];

  /// Takes a turn within [time]; null when none came in time. A turn held past
  /// [videoProbeHeldMost] is taken back first (see there): no timer waits on it.
  Future<_Turn?> take(Duration time) {
    final now = videoProbeClock();
    for (final turn in _held.toList()) {
      if (now - turn.since < videoProbeHeldMost) break;
      debugPrint(
        'a video reading still goes on after ${videoProbeHeldMost.inSeconds} s: '
        'its turn goes back',
      );
      give(turn);
    }
    if (_free > 0) {
      _free--;
      return Future.value(_hold());
    }
    final waiting = Completer<_Turn?>();
    _waiting.add(waiting);
    return waiting.future.timeout(
      time,
      onTimeout: () {
        // A turn handed over completes the wait at once, before this can come.
        _waiting.remove(waiting);
        return null;
      },
    );
  }

  _Turn _hold() {
    final turn = _Turn(videoProbeClock());
    _held.add(turn);
    return turn;
  }

  /// Gives [turn] back, once: the reading ended, or it was taken back already.
  void give(_Turn turn) {
    if (!_held.remove(turn)) return;
    while (_waiting.isNotEmpty) {
      final next = _waiting.removeAt(0);
      if (!next.isCompleted) {
        next.complete(_hold());
        return;
      }
    }
    _free++;
  }
}

/// Measures how long turns are held and readings waited for: a monotonic clock, which
/// the wall clock set forward or back (on waking, by NTP) does not move.
final _monotonic = Stopwatch()..start();

/// The time on [_monotonic]; tests put fake time in its place, with that of their timers.
@visibleForTesting
Duration Function() videoProbeClock = () => _monotonic.elapsed;

/// A turn to read a video, and when it was taken.
class _Turn {
  _Turn(this.since);

  /// When it was taken, on [videoProbeClock].
  final Duration since;
}

/// [f] as it goes: a video with its first picture and length, as this computer reads
/// them; null for what is no video, cannot be read, or is not read within
/// [videoProbeTime] (its turn included).
Future<UiVideo?> videoFor(OutgoingFile f) async {
  if (!isVideo(f.name)) return null;
  final started = videoProbeClock();
  final turn = await _turns.take(videoProbeTime);
  if (turn == null) return null;
  final Future<VideoProbe?> reading;
  try {
    // `then` makes it a future of a nullable probe whatever the reader returns.
    reading = videoProbe(f.path).then<VideoProbe?>((probe) => probe);
  } catch (_) {
    _turns.give(turn);
    return null;
  }
  unawaited(
    reading
        .then((_) {}, onError: (Object _) {})
        .whenComplete(() => _turns.give(turn)),
  );
  final left = videoProbeTime - (videoProbeClock() - started);
  final VideoProbe? probe;
  try {
    probe = await reading.timeout(
      left.isNegative ? Duration.zero : left,
      onTimeout: () => null,
    );
  } catch (_) {
    return null;
  }
  if (probe == null) return null;
  return UiVideo(
    thumb: probe.thumb,
    width: probe.width,
    height: probe.height,
    durationMs: probe.durationMs,
  );
}

/// Runs [work] on [items] with at most [atOnce] at a time; the results are in the order of
/// the items. Once [stop] says so, no more are started and their results are null. Once
/// one fails none is started either (the whole is given up on, as its caller does), and
/// the first failure is thrown when those running have ended.
Future<List<R?>> inPool<T, R>(
  List<T> items,
  int atOnce,
  Future<R> Function(T item) work, {
  bool Function()? stop,
}) async {
  final results = List<R?>.filled(items.length, null);
  var next = 0;
  var failed = false;
  Future<void> run() async {
    while (next < items.length && !failed && !(stop?.call() ?? false)) {
      final i = next++;
      try {
        results[i] = await work(items[i]);
      } catch (_) {
        failed = true;
        rethrow;
      }
    }
  }

  await Future.wait([for (var i = 0; i < atOnce; i++) run()]);
  return results;
}
