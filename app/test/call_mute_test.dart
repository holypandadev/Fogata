import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// A connection closed or replaced meanwhile: whatever is asked of it fails.
class _Gone implements RTCPeerConnection {
  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async =>
      throw StateError('the connection is closed');

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A connection whose four channels note what they are given.
class _Connection implements RTCPeerConnection {
  final channels = [for (var i = 0; i < 4; i++) _Transceiver(i)];

  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async => channels;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transceiver implements RTCRtpTransceiver {
  _Transceiver(int index) : mid = '$index';
  final _sender = _Sender();
  TransceiverDirection? direction;

  @override
  final String mid;

  @override
  RTCRtpSender get sender => _sender;

  @override
  Future<void> setDirection(TransceiverDirection direction) async =>
      this.direction = direction;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Sender implements RTCRtpSender {
  int replaced = 0;

  @override
  Future<void> replaceTrack(MediaStreamTrack? track) async => replaced++;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  final sent = <(String, String)>[];
  setUp(() {
    sent.clear();
    signalsForTests = (to, message) => sent.add((to, message['t'] as String));
  });
  tearDown(() => signalsForTests = null);

  // One connection closed as the microphone is turned off (made again as it is retried):
  // the others still stop receiving the voice, and everyone is told the microphone is off.
  test('a mute reaches every participant when one connection fails', () async {
    final good = _Connection();
    final call = Call(group: 'g', me: 'me', mic: true)
      ..peers['aaaaaaaa'] = (CallPeer('aaaaaaaa')..connection = _Gone())
      ..peers['bbbbbbbb'] = (CallPeer('bbbbbbbb')..connection = good);
    await call.setMic(false);
    expect(good.channels[0].direction, TransceiverDirection.RecvOnly);
    expect(good.channels[0]._sender.replaced, 1);
    // "me" sorts after both: each is asked for a new offer.
    expect(sent, contains(('bbbbbbbb', 'update')));
    expect(sent, containsAll([('aaaaaaaa', 'state'), ('bbbbbbbb', 'state')]));
  });

  test('the screen\'s sound turned off reaches every participant when one connection '
      'fails', () async {
    final good = _Connection();
    final call = Call(group: 'g', me: 'me', mic: false)
      ..peers['aaaaaaaa'] = (CallPeer('aaaaaaaa')..connection = _Gone())
      ..peers['bbbbbbbb'] = (CallPeer('bbbbbbbb')..connection = good);
    await call.setShareSound(false);
    expect(good.channels[3]._sender.replaced, 1);
    expect(sent, containsAll([('aaaaaaaa', 'state'), ('bbbbbbbb', 'state')]));
  });
}
