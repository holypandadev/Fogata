import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// What a connection's statistics list of the pictures on channel [mid]: [frames] decoded
/// and [bytes] received so far.
StatsReport video(Object? mid, num frames, num bytes, {String? id}) =>
    StatsReport(id ?? 'in-$mid', 'inbound-rtp', 0, {
      'kind': 'video',
      'mid': ?mid?.toString(),
      'framesDecoded': frames,
      'bytesReceived': bytes,
    });

/// Feeds [flood] a reading every [every] of the pictures on each channel, at [fps] frames
/// and [kbps] kbit/s for each reading in turn (the last repeated), from [start]; what it
/// said at each reading.
List<String?> readings(
  VideoFlood flood, {
  required Map<int, List<(num fps, num kbps)>> rates,
  int count = 20,
  Duration every = const Duration(seconds: 2),
  List<Duration>? spans,
}) {
  var at = DateTime(2026);
  final frames = <int, num>{};
  final bytes = <int, num>{};
  final said = <String?>[];
  for (var i = 0; i < count; i++) {
    final span = spans?[i % spans.length] ?? every;
    if (i > 0) {
      at = at.add(span);
      final seconds = span.inMicroseconds / Duration.microsecondsPerSecond;
      for (final MapEntry(key: mid, value: steps) in rates.entries) {
        final (fps, kbps) = steps[math(i - 1, steps.length)];
        frames[mid] = (frames[mid] ?? 0) + fps * seconds;
        bytes[mid] = (bytes[mid] ?? 0) + kbps * 1000 / 8 * seconds;
      }
    }
    said.add(
      flood.take([
        for (final mid in rates.keys)
          video(mid, frames[mid] ?? 0, bytes[mid] ?? 0),
      ], at),
    );
  }
  return said;
}

int math(int i, int length) => i < length ? i : length - 1;

void main() {
  test('honest copies at their most are never kept out', () {
    // A camera at its most for two people (60 fps, 1500 kbit/s) and a screen at its most
    // (60 fps, 20 Mbit/s), read every 2 s.
    expect(
      readings(
        VideoFlood(),
        rates: {
          1: [(60, cameraMaxKbps)],
          2: [(60, screenMaxKbps)],
        },
      ).nonNulls,
      isEmpty,
    );
    // Read again 1.1 s after a reading, as for a participant gone silent, and once
    // sooner than that.
    expect(
      readings(
        VideoFlood(),
        rates: {
          1: [(30, 1500)],
          2: [(60, 20000)],
        },
        spans: const [
          Duration(seconds: 2),
          Duration(milliseconds: 1100),
          Duration(milliseconds: 300),
          Duration(seconds: 2),
        ],
      ).nonNulls,
      isEmpty,
    );
  });

  test('a catch-up after a stall, or a key frame, is not a flood', () {
    // Nothing for 4 s, then all of it at once, then as before: one reading at more than
    // three times the limit, in frames and bytes.
    expect(
      readings(
        VideoFlood(),
        rates: {
          1: [(30, 1500), (0, 0), (0, 0), (400, 9000), (30, 1500)],
        },
      ).nonNulls,
      isEmpty,
    );
    // Key frames asked for again and again (losses): bytes well past the limit every
    // other reading, never two in a row.
    expect(
      readings(
        VideoFlood(),
        rates: {
          2: [
            for (var i = 0; i < 10; i++) ...[(60, 70000), (60, 15000)],
          ],
        },
      ).nonNulls,
      isEmpty,
    );
  });

  test(
    'pictures far faster than any copy sends, twice in a row, are a flood',
    () {
      final flood = VideoFlood();
      final said = readings(
        flood,
        rates: {
          1: [(30, 1500), (250, 1500)],
        },
        count: 4,
      );
      // The first reading above the limit is let through; the second is not.
      expect(said.take(3), everyElement(isNull));
      expect(
        said.last,
        'video on channel 1 at 250 frames a second, 1500 kbit/s',
      );
    },
  );

  test(
    'a camera sending far more bytes than any copy, twice in a row, is a flood',
    () {
      final said = readings(
        VideoFlood(),
        rates: {
          1: [(30, 8000)],
        },
        count: 3,
      );
      expect(
        said.last,
        'video on channel 1 at 30 frames a second, 8000 kbit/s',
      );
      // A screen may send that much.
      expect(
        readings(
          VideoFlood(),
          rates: {
            2: [(30, 8000)],
          },
        ).nonNulls,
        isEmpty,
      );
    },
  );

  test('a flood that pauses for a reading starts over', () {
    expect(
      readings(
        VideoFlood(),
        rates: {
          2: [
            for (var i = 0; i < 10; i++) ...[(500, 1000), (30, 1000)],
          ],
        },
      ).nonNulls,
      isEmpty,
    );
  });

  test(
    'a stream whose counts start again is not judged against the old one',
    () {
      final flood = VideoFlood();
      final at = DateTime(2026);
      expect(flood.take([video(1, 100000, 1e9)], at), isNull);
      // A new stream on the same channel (another connection's counts), then one that
      // counts from there at an honest rate.
      expect(
        flood.take([video(1, 10, 1000)], at.add(const Duration(seconds: 2))),
        isNull,
      );
      expect(
        flood.take([video(1, 70, 400000)], at.add(const Duration(seconds: 4))),
        isNull,
      );
      expect(
        flood.take([video(1, 130, 800000)], at.add(const Duration(seconds: 6))),
        isNull,
      );
    },
  );

  test(
    'every stream of a channel counts, the old one listed beside the new',
    () {
      final flood = VideoFlood();
      final at = DateTime(2026);
      String? read(int s, num frames) => flood.take([
        video(1, 5000, 1e6, id: 'old'),
        video(1, frames, 1e5 * s, id: 'new'),
      ], at.add(Duration(seconds: s)));
      expect(read(0, 0), isNull);
      expect(read(2, 400), isNull);
      expect(
        read(4, 800),
        'video on channel 1 at 200 frames a second, 800 kbit/s',
      );
    },
  );

  test('statistics that name no channel are held to a screen\'s limits', () {
    final flood = VideoFlood();
    final at = DateTime(2026);
    expect(flood.take([video(null, 0, 0)], at), isNull);
    for (var s = 2; s <= 10; s += 2) {
      expect(
        flood.take([
          video(null, 60 * s, 8000 * 125 * s),
        ], at.add(Duration(seconds: s))),
        isNull,
      );
    }
  });
}
