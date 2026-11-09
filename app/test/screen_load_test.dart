import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart' show StatsReport;
import 'package:fogata/src/call.dart';

final start = DateTime(2026, 11, 5, 9);

typedef Encoders =
    Map<
      String,
      ({bool cpu, double encodeSeconds, double? fps, double? captured})
    >;

/// A reading of [watchers] encoders, [after] seconds from [start], each having spent
/// [busy] of every second encoding so far, held back by the processor when [cpu], sending
/// [fps] frames a second (by default 2 when held back, 30 otherwise) of the [captured] ones
/// (30 by default); [unknown] leaves both rates out, as statistics may.
({Encoders encoders, DateTime now}) reading(
  int after, {
  int watchers = 3,
  double busy = 0.1,
  bool cpu = false,
  double? fps,
  double captured = 30,
  bool unknown = false,
}) => (
  encoders: {
    for (var i = 0; i < watchers; i++)
      'p$i': (
        cpu: cpu,
        encodeSeconds: busy * after,
        fps: unknown ? null : fps ?? (cpu ? 2.0 : 30.0),
        captured: unknown ? null : captured,
      ),
  },
  now: start.add(Duration(seconds: after)),
);

/// Feeds a reading every 2 s from [from] to [to] seconds; whether the level changed at any.
bool feed(
  ScreenLoad load,
  int from,
  int to, {
  int watchers = 3,
  double busy = 0.1,
  bool cpu = false,
  double? fps,
  double captured = 30,
  bool unknown = false,
}) {
  var changed = false;
  for (var t = from; t <= to; t += 2) {
    final r = reading(
      t,
      watchers: watchers,
      busy: busy,
      cpu: cpu,
      fps: fps,
      captured: captured,
      unknown: unknown,
    );
    changed |= load.take(encoders: r.encoders, now: r.now);
  }
  return changed;
}

void main() {
  test('a light screen stays at the full size and rate', () {
    final load = ScreenLoad();
    expect(feed(load, 0, 120), isFalse);
    expect(load.level, 0);
    expect(load.limits, (lines: null, fps: 60));
  });

  test('two readings held back by the processor lower it for everyone', () {
    final load = ScreenLoad();
    feed(load, 0, 10);
    final first = reading(12, cpu: true);
    expect(load.take(encoders: first.encoders, now: first.now), isFalse);
    final second = reading(14, cpu: true);
    expect(load.take(encoders: second.encoders, now: second.now), isTrue);
    // Captured at 30, the level at 30 frames sends the same as the full one: passed over.
    expect(load.level, 2);
    expect(load.limits.fps, 15);
    expect(load.lowered, isTrue);
  });

  test('a screen at 60 frames a second halves its rate a level at a time', () {
    final load = ScreenLoad()..capturedFps = 60;
    feed(load, 0, 2, cpu: true, captured: 60);
    expect(load.limits, (lines: null, fps: 30));
    expect(load.lowered, isTrue);
    // Still held back once that has settled: 15, then a smaller size.
    feed(load, 4, 16, cpu: true, captured: 60);
    expect(load.limits, (lines: null, fps: 15));
    feed(load, 18, 30, cpu: true, captured: 60);
    expect(load.limits, (lines: 720, fps: 15));
  });

  test('a screen at 60 frames a second comes back from 30 when 60 fits', () {
    final load = ScreenLoad()..capturedFps = 60;
    feed(load, 0, 2, cpu: true, captured: 60);
    expect(load.level, 1);
    // Three encoders at 0.2 of a core each at 30 frames: 1.2 cores at 60, under the
    // limit, so back up once light for 20 s, from the first reading at that rate. (From
    // 15, the same work would have had to fit four times over.)
    expect(feed(load, 4, 24, busy: 0.2, fps: 30, captured: 60), isFalse);
    expect(feed(load, 26, 26, busy: 0.2, fps: 30, captured: 60), isTrue);
    expect(load.level, 0);
    expect(load.lowered, isFalse);
  });

  test('a level that sends what the first does is not lowered', () {
    final load = ScreenLoad()..level = 1;
    expect(load.lowered, isFalse);
    load.capturedFps = 60;
    expect(load.lowered, isTrue);
  });

  test('encoders that keep too many cores busy lower it too', () {
    final load = ScreenLoad();
    // Three encoders each busy 0.6 of a second every second: 1.8 cores.
    feed(load, 0, 8, busy: 0.6);
    expect(load.level, greaterThan(0));
  });

  test('a lasting overload goes down to the last level and no further', () {
    final load = ScreenLoad();
    feed(load, 0, 200, cpu: true);
    expect(load.level, ScreenLoad.levels.length - 1);
    expect(load.limits, (lines: 540, fps: 5));
  });

  test(
    'it goes back up only after a light while, and waits longer if that fails',
    () {
      final load = ScreenLoad();
      feed(load, 0, 2, cpu: true);
      expect(load.level, 2);
      // Light from 4 s: not for 20 s yet.
      expect(feed(load, 4, 22), isFalse);
      expect(load.level, 2);
      expect(feed(load, 24, 24), isTrue);
      expect(load.level, 0);
      // Held back again once the change has settled: down, and the next try waits twice
      // as long.
      feed(load, 26, 34);
      feed(load, 36, 38, cpu: true);
      expect(load.level, 2);
      expect(load.wait, const Duration(seconds: 40));
      expect(feed(load, 40, 78), isFalse);
      expect(feed(load, 80, 80), isTrue);
      expect(load.level, 0);
    },
  );

  test('a busy reading, even under the limit, holds the level', () {
    final load = ScreenLoad();
    feed(load, 0, 2, cpu: true, busy: 0.3);
    expect(load.level, 2);
    // 3 × 0.3 = 0.9 cores: over half the limit, so no step up however long.
    expect(feed(load, 4, 200, busy: 0.3), isFalse);
    expect(load.level, 2);
  });

  test('a level up is tried only if it would fit', () {
    // Down to 720 lines at 15 fps.
    final load = ScreenLoad()..watchersAre(5);
    expect(load.level, 3);
    // Three encoders at 0.27 of a core each: 0.8 cores now, 1.8 at the full size.
    expect(feed(load, 0, 100, busy: 0.27), isFalse);
    expect(load.level, 3);
    // At 0.5 cores now, 1.1 at the full size: under the limit, so up after 20 s.
    final light = ScreenLoad()..watchersAre(5);
    feed(light, 0, 30, busy: 0.17);
    expect(light.level, 2);
    expect(ScreenLoad.costOf(2) / ScreenLoad.costOf(3), closeTo(2.25, 0.01));
  });

  test('WebRTC still saying "cpu" after a step does not walk it down', () {
    final load = ScreenLoad();
    feed(load, 0, 2, cpu: true);
    expect(load.level, 2);
    // WebRTC's own lowering lasts and keeps saying "cpu", but the screen goes out at the
    // level's 15 frames a second: nothing more is lowered (and, the encoders still busy,
    // nothing raised).
    expect(feed(load, 4, 200, cpu: true, fps: 15, busy: 0.15), isFalse);
    expect(load.level, 2);
  });

  test('a still screen, captured at a few frames, is not lowered', () {
    // Under Wayland a screen that does not change is captured at a few frames a second;
    // WebRTC may still say "cpu".
    final load = ScreenLoad();
    expect(feed(load, 0, 120, cpu: true, fps: 2, captured: 2), isFalse);
    expect(load.level, 0);
  });

  test('rates not told say nothing', () {
    final load = ScreenLoad();
    expect(feed(load, 0, 120, cpu: true, unknown: true), isFalse);
    expect(load.level, 0);
  });

  test('frames that rise again are no longer held back', () {
    final load = ScreenLoad();
    feed(load, 0, 0, cpu: true, fps: 5);
    // Held back, but each reading sends more than the last.
    var changed = false;
    for (var t = 2, fps = 6.0; t <= 20; t += 2, fps += 1) {
      final r = reading(t, cpu: true, fps: fps);
      changed |= load.take(encoders: r.encoders, now: r.now);
    }
    expect(changed, isFalse);
  });

  test('"cpu" stuck with idle encoders lets it go back up', () {
    final load = ScreenLoad()..watchersAre(5, now: start);
    expect(load.level, 3);
    // WebRTC says "cpu" and the screen goes out at the level's rate, but the encoders do
    // almost nothing (0.15 cores): its word is taken as stuck after 20 s of that, and the
    // level goes up once it has been light for 20 s more.
    expect(feed(load, 0, 38, cpu: true, fps: 15, busy: 0.05), isFalse);
    expect(feed(load, 40, 40, cpu: true, fps: 15, busy: 0.05), isTrue);
    expect(load.level, 2);
  });

  test('"cpu" is not taken for a while after a change', () {
    final load = ScreenLoad();
    feed(load, 0, 2, cpu: true);
    expect(load.level, 2);
    // Held back and slow, but within 12 s of the change.
    expect(feed(load, 4, 12, cpu: true), isFalse);
    expect(load.level, 2);
    // Still so after it: down a level.
    expect(feed(load, 14, 16, cpu: true), isTrue);
    expect(load.level, 3);
  });

  test("the rate a level costs is no more than the screen's", () {
    // Captured at 60 frames a second, the full rate costs four times 15; at 30, twice.
    expect(
      ScreenLoad.costOf(0, fps: 60) / ScreenLoad.costOf(2, fps: 60),
      closeTo(4, 0.01),
    );
    expect(ScreenLoad.costOf(0) / ScreenLoad.costOf(2), closeTo(2, 0.01));
    expect(
      ScreenLoad.costOf(0, fps: 15) / ScreenLoad.costOf(2, fps: 15),
      closeTo(1, 0.01),
    );
  });

  test('watchers that leave and come back do not lower it again', () {
    final load = ScreenLoad();
    expect(load.watchersAre(3, now: start), isTrue);
    expect(load.level, 2);
    expect(feed(load, 0, 22), isTrue);
    expect(load.level, 0);
    // One leaves and comes back: as many as before.
    expect(load.watchersAre(2), isFalse);
    expect(load.watchersAre(3), isFalse);
    expect(load.level, 0);
    // A fourth is more than ever since the change.
    expect(load.watchersAre(4), isTrue);
    expect(load.level, 2);
  });

  test('more watchers start it lower', () {
    expect(ScreenLoad.startFor(1), 0);
    expect(ScreenLoad.startFor(2), 0);
    expect(ScreenLoad.startFor(3), 2);
    expect(ScreenLoad.startFor(5), 3);
    final load = ScreenLoad();
    expect(load.watchersAre(2), isFalse);
    expect(load.watchersAre(5), isTrue);
    expect(load.limits, (lines: 720, fps: 15));
    // Fewer watchers do not raise it by themselves; the processor does, in time.
    expect(load.watchersAre(1), isFalse);
    expect(load.level, 3);
  });

  test('the same watchers again do not undo a level up', () {
    final load = ScreenLoad();
    expect(load.watchersAre(3), isTrue);
    expect(load.level, 2);
    var t = 0;
    bool readingWith(int watchers) {
      final more = load.watchersAre(watchers);
      final r = reading(t, watchers: watchers);
      t += 2;
      return load.take(encoders: r.encoders, now: r.now) || more;
    }

    // Light for 20 s: up to the full screen, and it stays there with the same three.
    for (var i = 0; i <= 10; i++) {
      readingWith(3);
    }
    expect(load.level, 0);
    for (var i = 0; i < 30; i++) {
      expect(readingWith(3), isFalse);
    }
    expect(load.level, 0);
    // A fourth comes: more watchers, so down again to where four start.
    expect(readingWith(4), isTrue);
    expect(load.level, 2);
  });

  test('a new share starts over', () {
    final load = ScreenLoad();
    feed(load, 0, 40, cpu: true);
    load.reset();
    expect(load.level, 0);
    expect(load.wait, const Duration(seconds: 20));
  });

  test('the encoder is read from the screen channel only', () {
    StatsReport out(String mid, Map<String, dynamic> values) =>
        StatsReport('o$mid', 'outbound-rtp', 0, {'mid': mid, ...values});
    expect(screenEncoder([]), isNull);
    expect(
      screenEncoder([
        out('1', {'framesEncoded': 9, 'qualityLimitationReason': 'cpu'}),
      ]),
      isNull,
    );
    expect(
      screenEncoder([
        out('2', {'framesEncoded': 0}),
      ]),
      isNull,
    );
    expect(
      screenEncoder([
        out('2', {
          'framesEncoded': 90,
          'totalEncodeTime': 1.5,
          'qualityLimitationReason': 'cpu',
        }),
      ]),
      (cpu: true, encodeSeconds: 1.5, fps: null, captured: null),
    );
    expect(
      screenEncoder([
        out('2', {
          'framesEncoded': 90,
          'totalEncodeTime': 0.4,
          'qualityLimitationReason': 'bandwidth',
        }),
      ])?.cpu,
      isFalse,
    );
  });
}
