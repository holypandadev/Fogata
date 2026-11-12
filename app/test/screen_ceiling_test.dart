import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// A connection whose channels record what is asked of their senders.
class _Connection implements RTCPeerConnection {
  final channels = [for (var i = 0; i < 4; i++) _Sender()];

  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async => [
    for (final sender in channels) _Transceiver(sender),
  ];

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transceiver implements RTCRtpTransceiver {
  _Transceiver(this.sender);

  @override
  final RTCRtpSender sender;

  @override
  String get mid => '';

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Sender implements RTCRtpSender {
  RTCRtpEncoding? set;

  @override
  RTCRtpParameters get parameters =>
      RTCRtpParameters(encodings: [RTCRtpEncoding()]);

  @override
  Future<bool> setParameters(RTCRtpParameters parameters) async {
    set = parameters.encodings!.single;
    return true;
  }

  @override
  Future<void> replaceTrack(MediaStreamTrack? track) async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// The size a screen of [width] by [height] goes out at with [watchers] watching, at the
/// [chosen] quality's lines (null: as captured).
(int, int) out(double width, double height, int watchers, {int? chosen}) {
  final scale = screenScale(
    width,
    height,
    lines: cappedLines(chosen, screenCeiling(watchers)),
  );
  return ((width / scale).round(), (height / scale).round());
}

void main() {
  test('the more watch, the fewer lines the screen may go out at', () {
    expect(screenCeiling(0), isNull);
    expect(screenCeiling(1), isNull);
    expect(screenCeiling(2), isNull);
    expect(screenCeiling(3), 1080);
    expect(screenCeiling(4), 1080);
    expect(screenCeiling(5), 720);
    expect(screenCeiling(19), 720);
    // Never more lines with more watching.
    int lines(int watchers) => screenCeiling(watchers) ?? 1 << 30;
    for (var n = 1; n < 20; n++) {
      expect(lines(n + 1), lessThanOrEqualTo(lines(n)));
    }
  });

  test('the ceiling never sends more than chosen', () {
    expect(cappedLines(null, null), isNull);
    expect(cappedLines(720, null), 720);
    expect(cappedLines(1080, null), 1080);
    expect(cappedLines(null, 1080), 1080);
    expect(cappedLines(720, 1080), 720);
    expect(cappedLines(1080, 720), 720);
    expect(cappedLines(null, 720), 720);
  });

  test('one or two watching get what was chosen, as before', () {
    for (final watchers in [1, 2]) {
      expect(out(3840, 2160, watchers), (3840, 2160));
      expect(out(3840, 2160, watchers, chosen: 1080), (1920, 1080));
      expect(out(1600, 900, watchers, chosen: 720), (1280, 720));
    }
  });

  test('more watching get a smaller box, never larger than the source', () {
    // A 4K screen shared at its own size: 1080 lines for three or four, 720 from five.
    expect(out(3840, 2160, 3), (1920, 1080));
    expect(out(3840, 2160, 4), (1920, 1080));
    expect(out(3840, 2160, 5), (1280, 720));
    // The lab's 1600×900 screen: as it is for three, 720 lines from five.
    expect(out(1600, 900, 3, chosen: 1080), (1600, 900));
    expect(out(1600, 900, 7, chosen: 1080), (1280, 720));
    // 720 chosen stays 720, however few watch.
    expect(out(2560, 1440, 3, chosen: 720), (1280, 720));
    // A screen smaller than the box is never made larger.
    expect(out(1280, 720, 7), (1280, 720));
    expect(out(1024, 768, 7, chosen: 1080), (960, 720));
    // An ultrawide keeps its shape within the 16:9 box, as a quality does.
    expect(out(3440, 1440, 5), (1280, 536));
  });

  test(
    'watchers coming make the screen smaller on every connection at once',
    () async {
      final call = Call(group: 'g', me: 'me')..sharing = true;
      // A 4K screen shared at its own size.
      call.screenQuality = const ScreenQuality(lines: null);
      call.localScreen.value = RTCVideoValue.empty.copyWith(
        width: 3840,
        height: 2160,
      );
      await call.rescaleNow();
      expect(call.screenScaleSent, 1);
      final connections = <String, _Connection>{};
      for (var i = 0; i < 6; i++) {
        final id = 'p$i';
        final connection = connections[id] = _Connection();
        call.peers[id] = CallPeer(id)..connection = connection;
      }
      RTCRtpEncoding? screenTo(String id) => connections[id]!.channels[2].set;
      Future<void> watch(String id, {bool on = true}) async {
        await call.handleSignal(id, {'t': 'state', 'watch': on ? 'me' : ''});
        await pumpEventQueue();
      }

      await watch('p0');
      await watch('p1');
      // One or two: as captured, to each as they come.
      expect(call.watchers, 2);
      expect(call.screenScaleSent, 1);
      expect(screenTo('p1')!.scaleResolutionDownBy, 1);
      expect(call.screenLoad.ceiling, isNull);

      // A third: 1080 lines for everyone, the one who came included.
      await watch('p2');
      expect(call.screenScaleSent, 2);
      for (final id in ['p0', 'p1', 'p2']) {
        expect(screenTo(id)!.scaleResolutionDownBy, 2);
        // And 15 frames a second, where three start.
        expect(screenTo(id)!.maxFramerate, 15);
      }
      // Who does not watch is told nothing.
      expect(screenTo('p5'), isNull);

      // A fifth: 720 lines, and 540 to start with, for the processor.
      await watch('p3');
      await watch('p4');
      expect(call.screenLoad.ceiling, 720);
      expect(call.screenScaleSent, 3);
      for (final id in ['p0', 'p1', 'p2', 'p3', 'p4']) {
        expect(screenTo(id)!.scaleResolutionDownBy, 4);
        expect(screenTo(id)!.maxFramerate, 15);
      }

      // One leaves and comes back at once: nothing moves.
      await watch('p4', on: false);
      expect(call.screenLoad.ceiling, 720);
      expect(call.screenScaleSent, 3);
      await watch('p4');
      expect(screenTo('p4')!.scaleResolutionDownBy, 4);
    },
  );

  test(
    'a new quality chosen while many watch stays within the ceiling',
    () async {
      final call = Call(group: 'g', me: 'me')..sharing = true;
      call.localScreen.value = RTCVideoValue.empty.copyWith(
        width: 3840,
        height: 2160,
      );
      for (var i = 0; i < 5; i++) {
        call.peers['p$i'] = CallPeer('p$i')..connection = _Connection();
      }
      for (var i = 0; i < 5; i++) {
        await call.handleSignal('p$i', {'t': 'state', 'watch': 'me'});
      }
      expect(call.screenScaleSent, 3);
      // 1080 chosen: still 720 lines while five watch.
      await call.setScreenQuality(const ScreenQuality(lines: 1080));
      expect(call.screenScaleSent, 3);
      // 720 chosen: the same.
      await call.setScreenQuality(const ScreenQuality(lines: 720));
      expect(call.screenScaleSent, 3);
    },
  );
}
