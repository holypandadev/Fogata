import 'package:flutter_test/flutter_test.dart';

import '../integration_test/output_swap_verdict.dart';

void main() {
  const lost = ['Could not use the microphone.'];

  /// A stretch whose readings each sent [voice] packets and read [read] seconds.
  SwapSegment segment(
    String name,
    List<double> voice, {
    List<double?>? read,
    bool micOn = true,
  }) => (
    name: name,
    micOn: micOn,
    readings: [
      for (final (i, v) in voice.indexed)
        (voice: v, read: read == null ? 2.0 : read[i], level: 0.1, took: 2.0),
    ],
  );

  /// The test's run as it goes: the other output, the microphone off and on twice in it,
  /// and the first again.
  List<SwapSegment> run({
    List<double> other = const [9, 9, 50, 50, 50],
    List<double> on1 = const [0, 0, 50, 50, 50],
    List<double> on2 = const [0, 9, 50, 50, 50],
    List<double> again = const [9, 0, 50, 50],
    List<double?>? read,
  }) => [
    segment('first', [50, 50, 50]),
    segment('the other output', other, read: read),
    segment('microphone off', [0, 0], micOn: false),
    segment('the other output, the microphone on again (1)', on1),
    segment('microphone off', [0, 0], micOn: false),
    segment('the other output, the microphone on again (2)', on2),
    segment('the first again output', again),
  ];

  String? verdict(
    List<SwapSegment> segments, {
    bool after = true,
    List<String> told = const [],
  }) => outputSwapVerdict(
    segments: segments,
    micOnAfter: after,
    told: told,
    micLost: lost,
  );

  test('voice and sound read all along pass; the two readings after a change and the '
      'microphone off are not judged', () {
    expect(verdict(run()), isNull);
  });

  test('the microphone lost with the other output fails', () {
    // As on 2026-11-18: voice only in the reading of the change, none after, the
    // microphone turned off and the person told.
    final why = verdict(
      run(other: [18, 0, 0, 0, 0, 0], again: [0, 0, 0, 0, 0]),
      after: false,
      told: [
        'Could not use the microphone. Check that it is connected and allowed in '
            'System Settings › Privacy & Security.',
      ],
    );
    expect(
      why,
      contains('no voice sent in 4 of 4 readings with the other output'),
    );
    expect(
      why,
      contains('no voice sent in 3 of 3 readings with the first again output'),
    );
    expect(why, contains('the microphone was turned off'));
    expect(why, contains('told: Could not use the microphone.'));
  });

  test('voice not back after the microphone is turned on again fails', () {
    expect(
      verdict(run(on2: [0, 0, 0, 3, 0])),
      'no voice sent in 2 of 3 readings with the other output, the microphone on '
      'again (2)',
    );
  });

  test('a microphone that reads no sound fails though packets go out', () {
    // A dead input still sends Opus's packets of silence.
    expect(
      verdict(run(read: [2, 2, 0, 0, 0])),
      'no sound read in 3 of 3 readings with the other output',
    );
  });

  SwapSegment levels(List<double> levels) => (
    name: 'the other output, the microphone on again (1)',
    micOn: true,
    readings: [
      for (final l in levels) (voice: 50.0, read: 2.0, level: l, took: 2.0),
    ],
  );

  test('a level exactly 0 in every judged reading of a stretch fails', () {
    // A dead input that gives zeros: its samples are counted all the same.
    final dead = run();
    dead[3] = levels([0.01, 0.01, 0, 0, 0, 0, 0]);
    expect(
      verdict(dead),
      'the level was 0 in all 5 readings judged with the other output, the microphone '
      'on again (1)',
    );
  });

  test('a level of 0 for a while, or in a short stretch, passes', () {
    // A microphone that gates noise, or muted on the device for a moment.
    final gated = run();
    gated[3] = levels([0, 0, 0, 0, 0, 0, 0, 0.001]);
    expect(verdict(gated), isNull);
    final short = run();
    short[3] = levels([0.01, 0.01, 0, 0, 0, 0]);
    expect(verdict(short), isNull);
  });

  test('a stretch with the pair refused for the test is let take longer', () {
    final refused = run();
    refused[5] = (
      name:
          'the other output, the microphone on again (2), $swapRefusedForTest',
      micOn: true,
      readings: [
        for (final v in [0.0, 0.0, 0.0, 11.0, 50.0, 50.0])
          (voice: v, read: v > 0 ? 2.0 : 0.0, level: 0.01, took: 2.0),
      ],
    );
    expect(verdict(refused), isNull);
  });

  test(
    'a restart of the recording is asked only where the refusal lost it',
    () {
      // The numbers of two device runs: with the unmute refused the recording went on (the
      // first reading after any unmute has no packets), with the start refused it was lost.
      const name =
          'the other output, the microphone on again (2), $swapRefusedForTest';
      expect(
        swapRecordingLost(
          segment(
            name,
            [0, 88, 99, 77, 99, 100, 100, 100, 100, 102],
            read: [1.8, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.1, 2.0],
          ),
        ),
        isFalse,
      );
      expect(
        swapRecordingLost(
          segment(
            name,
            [0, 0, 0, 0, 73, 86, 100, 101, 101, 100],
            read: [0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0],
          ),
        ),
        isTrue,
      );
    },
  );

  test('devices not those chosen 6 s after a choice fail', () {
    expect(
      outputSwapVerdict(
        segments: run(),
        micOnAfter: true,
        told: const [],
        micLost: lost,
        notApplied: const ['playing on default, not monitor'],
      ),
      'playing on default, not monitor',
    );
  });

  test('a reading that spans no time is not judged', () {
    // As on a device run: the last reading, taken as the measure ended, read 0 s.
    final ended = run();
    final last = ended.last;
    ended[ended.length - 1] = (
      name: last.name,
      micOn: true,
      readings: [
        ...last.readings,
        (voice: 11.0, read: 0.0, level: 0.002, took: 0.05),
      ],
    );
    expect(verdict(ended), isNull);
  });

  test('recording with the microphone off fails', () {
    expect(
      outputSwapVerdict(
        segments: run(),
        micOnAfter: true,
        told: const [],
        micLost: lost,
        recordingOff: const ['at the end of a stretch with it off (1)'],
      ),
      'still recording with the microphone off, at the end of a stretch with it off '
      '(1)',
    );
  });

  test('a stretch too short to judge fails', () {
    expect(
      verdict(run(on1: [50, 50])),
      'no reading with the other output, the microphone on again (1) after the change',
    );
  });

  test('the notice alone fails', () {
    expect(
      verdict(run(), told: ['Could not use the microphone. Allow it.']),
      'told: Could not use the microphone. Allow it.',
    );
  });

  test('an output that cannot be used, told as such, passes', () {
    expect(
      verdict(
        run(),
        told: [
          'Sound cannot play on Monitor with this microphone. It plays on Headset.',
        ],
      ),
      isNull,
    );
  });

  test('a microphone silent from the start cannot be judged', () {
    final silent = run();
    silent[0] = segment('first', [50, 50], read: [0, 0]);
    expect(verdict(silent), contains('nothing to judge'));
    expect(verdict(const []), contains('nothing to judge'));
  });
}
