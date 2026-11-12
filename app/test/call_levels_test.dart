import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// A connection whose statistics take until [answer] completes, counting the readings.
class _Slow implements RTCPeerConnection {
  final answer = Completer<List<StatsReport>>();
  int readings = 0;

  @override
  Future<List<StatsReport>> getStats([MediaStreamTrack? track]) {
    readings++;
    return answer.future;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Track implements MediaStreamTrack {
  _Track(this.id);

  @override
  final String id;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Mic implements MediaStream {
  _Mic(this.track);
  final MediaStreamTrack track;

  @override
  List<MediaStreamTrack> getAudioTracks() => [track];

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A connection whose statistics answer by what is asked: [narrow] for a track's own
/// (null: the track is not found), [whole] for the whole connection's.
class _Stats implements RTCPeerConnection {
  _Stats({this.narrow = const {}, this.whole = const []});

  final Map<String, List<StatsReport>> narrow;
  final List<StatsReport> whole;
  final asked = <String?>[];

  @override
  Future<List<StatsReport>> getStats([MediaStreamTrack? track]) async {
    asked.add(track?.id);
    if (track == null) return whole;
    final reports = narrow[track.id];
    if (reports == null) throw StateError('getStats: track not found');
    return reports;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// What the receiver of a voice lists: its stream, with what it references.
List<StatsReport> _voiceIn(double level, {String mid = '0'}) => [
  StatsReport('IT01A1', 'inbound-rtp', 0, {
    'kind': 'audio',
    'mid': mid,
    'audioLevel': level,
  }),
  StatsReport('CIT01_111', 'codec', 0, {'mimeType': 'audio/opus'}),
];

/// What the sender of the microphone lists: its stream and its source.
List<StatsReport> _micOut(double level, String mic) => [
  StatsReport('OT01A2', 'outbound-rtp', 0, {
    'kind': 'audio',
    'mid': '0',
    'mediaSourceId': 'SA3',
  }),
  StatsReport('SA3', 'media-source', 0, {
    'kind': 'audio',
    'trackIdentifier': mic,
    'audioLevel': level,
  }),
];

void main() {
  test('voiceLevel reads voices only, the loudest', () {
    expect(voiceLevel(const []), isNull);
    expect(voiceLevel(_voiceIn(0.01)), 0.01);
    expect(voiceLevel([..._voiceIn(0.01), ..._voiceIn(0.4)]), 0.4);
    // The sound of a shared screen is not someone talking.
    expect(voiceLevel(_voiceIn(0.4, mid: '3')), isNull);
  });

  test('micLevel reads this device\'s microphone only', () {
    expect(micLevel(const [], 'mic'), isNull);
    expect(micLevel(_micOut(0.3, 'mic'), 'mic'), 0.3);
    expect(micLevel(_micOut(0.3, 'screen-sound'), 'mic'), isNull);
    expect(micLevel(_micOut(0.3, 'other'), null), 0.3);
  });

  test('who talks is read from the voice\'s receiver and the microphone\'s sender alone', () async {
    final mic = _Track('mic');
    final first = _Stats(
      narrow: {'voice-a': _voiceIn(0.5), 'mic': _micOut(0.3, 'mic')},
    );
    final second = _Stats(narrow: {'voice-b': _voiceIn(0.01)});
    final call = Call(group: 'g', me: 'me', mic: true)
      ..holdForTests(streams: [_Mic(mic)]);
    final a = CallPeer('aaaaaaaa')
      ..connection = first
      ..connected = true
      ..mic = true;
    final b = CallPeer('bbbbbbbb')
      ..connection = second
      ..connected = true
      ..mic = true;
    a.voiceForTests = _Track('voice-a');
    b.voiceForTests = _Track('voice-b');
    call.peers
      ..['aaaaaaaa'] = a
      ..['bbbbbbbb'] = b;
    await call.readLevelsNow();
    expect(a.speaking, isTrue);
    expect(b.speaking, isFalse);
    expect(call.speaking, isTrue);
    // No whole report, and this device's own level from one connection only.
    expect(first.asked, ['voice-a', 'mic']);
    expect(second.asked, ['voice-b']);
  });

  test('a whole report is read while the narrow one cannot tell', () async {
    final mic = _Track('mic');
    // The voice's receiver lists nothing yet, and the microphone is not on a sender.
    final connection = _Stats(
      narrow: {'voice': const []},
      whole: [..._voiceIn(0.5), ..._micOut(0.3, 'mic')],
    );
    final call = Call(group: 'g', me: 'me', mic: true)
      ..holdForTests(streams: [_Mic(mic)]);
    final peer = CallPeer('aaaaaaaa')
      ..connection = connection
      ..connected = true
      ..mic = true;
    peer.voiceForTests = _Track('voice');
    call.peers['aaaaaaaa'] = peer;
    await call.readLevelsNow();
    expect(peer.speaking, isTrue);
    expect(call.speaking, isTrue);
    expect(connection.asked, ['voice', null, 'mic', null]);
  });

  test('a voice track of a connection made before is not asked for', () async {
    final old = _Stats(narrow: {'voice': _voiceIn(0.5)});
    final now = _Stats(whole: _voiceIn(0.5));
    final call = Call(group: 'g', me: 'me', mic: false);
    final peer = CallPeer('aaaaaaaa')
      ..connection = old
      ..connected = true
      ..mic = true;
    peer.voiceForTests = _Track('voice');
    peer.connection = now;
    call.peers['aaaaaaaa'] = peer;
    await call.readLevelsNow();
    expect(peer.speaking, isTrue);
    expect(old.asked, isEmpty);
    expect(now.asked, [null]);
  });

  test(
    'a reading of who talks that takes long is not overtaken by the next',
    () async {
      final slow = _Slow();
      final call = Call(group: 'g', me: 'me', mic: false)
        ..peers['aaaaaaaa'] = (CallPeer('aaaaaaaa')
          ..connection = slow
          ..connected = true
          ..mic = true);
      final first = call.readLevelsNow();
      await pumpEventQueue();
      // Two more ticks of the 600 ms timer while the first still waits.
      await call.readLevelsNow();
      await call.readLevelsNow();
      expect(slow.readings, 1);
      slow.answer.complete([
        StatsReport('a', 'inbound-rtp', 0, {'audioLevel': 0.5, 'mid': '0'}),
      ]);
      await first;
      expect(call.peers['aaaaaaaa']!.speaking, isTrue);
      // Done, the next tick reads again.
      unawaited(call.readLevelsNow());
      await pumpEventQueue();
      expect(slow.readings, 2);
    },
  );
}
