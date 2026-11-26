part of '../call.dart';

/// For tests only: with `FOGATA_TEST_TAP_DIR=<folder>`, each connection's received sound
/// statistics, read every 100 ms for the connection's life, go to
/// `<folder>/stats-<role>-<participant>.jsonl` (role: FOGATA_ROLE, else the process id),
/// one line per reading: `asked` and `got` (epoch ms, as the reading was asked and as it
/// came) and `inbound`, the values of every received sound stream (`inbound-rtp` of kind
/// audio: NetEq's removedSamplesForAcceleration, insertedSamplesForDeceleration,
/// concealedSamples, jitterBufferDelay, totalAudioEnergy and the rest). The other half
/// of the taps, the sound itself, is in flutter_webrtc's fogata_test_tap.h. Without the
/// variable nothing is read or written.
final String _tapDir = Platform.environment['FOGATA_TEST_TAP_DIR'] ?? '';

/// Reads [connection]'s statistics every 100 ms while [current] says it is still the
/// participant's; a reading not back yet makes the next one wait.
void _tapStats(
  RTCPeerConnection connection,
  String participant,
  bool Function() current,
) {
  if (_tapDir.isEmpty) return;
  final role = Platform.environment['FOGATA_ROLE'] ?? 'pid$pid';
  final who = participant.length > 8
      ? participant.substring(0, 8)
      : participant;
  final IOSink out;
  try {
    out = File('$_tapDir/stats-$role-$who.jsonl')
        .openWrite(mode: FileMode.append);
  } catch (e) {
    _trace('statistics tap: $e');
    return;
  }
  var reading = false;
  var closed = false;
  // A folder that cannot be written to tells only here.
  unawaited(
    out.done.catchError((Object e) {
      closed = true;
      _trace('statistics tap: $e');
    }),
  );
  double now() => DateTime.now().microsecondsSinceEpoch / 1000;
  void write(Map<String, Object?> line) {
    if (closed) return;
    try {
      out.writeln(jsonEncode(line, toEncodable: (o) => o.toString()));
    } catch (_) {}
  }

  Timer.periodic(const Duration(milliseconds: 100), (timer) async {
    if (closed) {
      timer.cancel();
      return;
    }
    if (!current()) {
      timer.cancel();
      closed = true;
      unawaited(out.close().catchError((Object _) {}));
      return;
    }
    if (reading) return;
    reading = true;
    final asked = now();
    try {
      final reports = await connection.getStats().timeout(
        const Duration(seconds: 2),
      );
      final got = now();
      write({
        'asked': asked,
        'got': got,
        'inbound': [
          for (final r in reports)
            if (r.type == 'inbound-rtp' &&
                (r.values['kind'] ?? r.values['mediaType']) == 'audio')
              {'id': r.id, 'timestamp': r.timestamp, ...r.values},
        ],
      });
    } catch (e) {
      write({'asked': asked, 'error': '$e'});
    } finally {
      reading = false;
    }
  });
}
