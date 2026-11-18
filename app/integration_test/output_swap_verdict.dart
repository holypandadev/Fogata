/// One reading of FOGATA_TEST_OUTPUT_SWAP's measure (every 2 s): the voice packets this
/// copy sent, every connection's together, the seconds of sound its microphone read in
/// that time and the loudest level seen (null when not known), and the seconds the
/// microphone's reading spans ([took], null when not known).
typedef SwapReading = ({
  double voice,
  double? read,
  double? level,
  double? took,
});

/// A reading that spans less than this is not judged: the last one, taken as the measure
/// ends, has no time in it.
const swapShortest = 1.0;

/// A stretch of the measure between two changes: an output chosen, the microphone turned
/// off or on. [name] says which ('first', 'other', 'other, on again 1', 'first again').
typedef SwapSegment = ({String name, bool micOn, List<SwapReading> readings});

/// The readings after a change not judged: the engine is made again then, and a failure,
/// the look 500 ms later and a fall back can take over 2 s on a slow machine.
const swapExcused = 2;

/// A stretch fails when the level is exactly 0 in every judged reading of it, from this
/// many: a microphone that gates noise, or is muted on the device for a moment, reads 0
/// now and then.
const swapSilentLevels = 5;

/// A stretch whose name has this saw the pair refused for the test (FOGATA_TEST_REFUSE=
/// unmute): the fall back (a new engine, the recording started again by the call at its
/// 2 s readings) is let take [swapExcusedRefused] readings.
const swapRefusedForTest = 'the pair refused for the test';
const swapExcusedRefused = 4;

/// Whether [segment] (one with [swapRefusedForTest]) lost the recording, so that the call
/// had to start it again. A start that failed ([startFailed], FOGATA_TEST_REFUSE=start)
/// always loses it. Otherwise (=unmute), a reading past the verdict's own allowance for
/// the stretch ([swapExcusedRefused], among those spanning [swapShortest] or more) that
/// read no sound from the microphone: a slow change to the system's devices can read
/// nothing before that. The packets are not looked at: the first reading after any unmute
/// has none.
bool swapRecordingLost(SwapSegment segment, {required bool startFailed}) =>
    startFailed ||
    segment.readings
        .where((r) => r.took == null || r.took! >= swapShortest)
        .skip(swapExcusedRefused)
        .any((r) => r.read != null && !(r.read! > 0));

/// FOGATA_TEST_OUTPUT_SWAP's verdict (call_test.dart's _OutputSwap): why the copy lost its
/// microphone across the changes of output or its own off and on, or recorded with it
/// off, or null when neither. A voice engine that loses the microphone sends no voice from
/// then on, and a microphone recording from a dead input still sends packets in silence
/// (Opus DTX) and counts the samples it reads, zeros included: so in every stretch with
/// the microphone on, every reading after the first [swapExcused] must carry voice and
/// sound read, there must be one, and the level must not be exactly 0 in every judged
/// reading of a stretch of [swapSilentLevels] or more (a room is never that quiet: a
/// microphone left alone reads about 0.001). The first stretch, before any change, needs one reading
/// with voice and sound read, to show there is something to judge. A reading spanning
/// less than [swapShortest] is left out first. [micOnAfter] is whether the call had the
/// microphone on at the end; [recordingOff] the moments the voice engine still recorded
/// with the microphone off (the end of each stretch with it off, after the call ended);
/// [notApplied] what the engine's facts showed against the devices chosen (another output
/// in use while the pair could be put together, a notice given then, a fall back that did
/// not happen); [told] what the person was told, and [micLost] the texts that say the
/// microphone could not be used.
String? outputSwapVerdict({
  required List<SwapSegment> segments,
  required bool micOnAfter,
  required Iterable<String> told,
  required Iterable<String> micLost,
  Iterable<String> recordingOff = const [],
  Iterable<String> notApplied = const [],
}) {
  bool heard(SwapReading r) => r.voice > 0 && (r.read == null || r.read! > 0);
  bool spans(SwapReading r) => r.took == null || r.took! >= swapShortest;
  segments = [
    for (final s in segments)
      (
        name: s.name,
        micOn: s.micOn,
        readings: s.readings.where(spans).toList(),
      ),
  ];
  final first = segments.firstOrNull;
  if (first == null || !first.micOn || !first.readings.any(heard)) {
    return 'the microphone sent nothing before the output changed: nothing to judge';
  }
  final why = <String>[];
  for (final (i, segment) in segments.indexed) {
    if (!segment.micOn) continue;
    final excused = i == 0
        ? 0
        : segment.name.contains(swapRefusedForTest)
        ? swapExcusedRefused
        : swapExcused;
    final judged = segment.readings.skip(excused).toList();
    if (judged.length >= swapSilentLevels &&
        judged.every((r) => r.level == 0)) {
      why.add(
        'the level was 0 in all ${judged.length} readings judged with ${segment.name}',
      );
    }
    if (i == 0) continue;
    if (judged.isEmpty) {
      why.add('no reading with ${segment.name} after the change');
      continue;
    }
    final silent = judged.where((r) => !(r.voice > 0)).length;
    final unread = judged.where((r) => r.read != null && !(r.read! > 0)).length;
    if (silent > 0) {
      why.add(
        'no voice sent in $silent of ${judged.length} readings with ${segment.name}',
      );
    }
    if (unread > 0) {
      why.add(
        'no sound read in $unread of ${judged.length} readings with ${segment.name}',
      );
    }
  }
  if (!micOnAfter) why.add('the microphone was turned off');
  for (final what in notApplied) {
    why.add(what);
  }
  for (final when in recordingOff) {
    why.add('still recording with the microphone off, $when');
  }
  final lost = told.where(
    (t) => micLost.any((m) => m.isNotEmpty && t.contains(m)),
  );
  if (lost.isNotEmpty) why.add('told: ${lost.join(' | ')}');
  return why.isEmpty ? null : why.join('; ');
}
