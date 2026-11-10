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

void main() {
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
