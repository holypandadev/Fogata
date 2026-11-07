import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// A connection whose channels are read while someone else comes into the room.
class _Joining implements RTCPeerConnection {
  _Joining(this.onRead);
  final void Function() onRead;

  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async {
    onRead();
    return [];
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  for (final (what, change) in <(String, Future<void> Function(Call))>[
    ('muting', (call) => call.setMic(false)),
    ('the screen sound', (call) => call.setShareSound(false)),
  ]) {
    test('$what goes on when someone joins meanwhile', () async {
      final call = Call(group: 'g', me: 'me', mic: true);
      var joined = 0;
      for (final id in ['ana', 'bea']) {
        call.peers[id] = CallPeer(id)
          ..connection = _Joining(() {
            joined++;
            call.peers['new$joined'] = CallPeer('new$joined');
          });
      }
      await change(call);
      // Both connections were reached, and the newcomers are kept.
      expect(joined, 2);
      expect(call.peers.keys, containsAll(['ana', 'bea', 'new1', 'new2']));
    });
  }
}
