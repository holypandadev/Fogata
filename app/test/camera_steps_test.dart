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
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  test('two people keep the camera as it is', () {
    for (final people in [1, 2]) {
      final step = cameraStep(people);
      expect(step.lines, isNull);
      expect(step.kbps, cameraMaxKbps);
      expect(cameraScale(720, step), 1);
      expect(cameraScale(1080, step), 1);
      expect(cameraScale(1080, step, width: 1920), 1);
      // A camera larger than Full HD goes out at most that large, as more is refused.
      expect(cameraScale(2160, step, width: 3840), 2);
      expect(cameraScale(1080, step, width: 2560), closeTo(1.33, 0.01));
    }
  });

  test('a screen as captured goes out at most as large as is drawn', () {
    expect(screenScale(3840, 2160, lines: null), 1);
    expect(screenScale(7680, 4320, lines: null), 1);
    final wide = screenScale(11520, 2160, lines: null);
    expect(wide, closeTo(11520 / 8192, 0.001));
    expect(
      fairPicture(2, (11520 / wide).floor(), (2160 / wide).floor()),
      isTrue,
    );
  });

  test('a larger call sends a smaller camera, up to 3 Mbit/s in all', () {
    var last = cameraStep(2);
    // A room takes 8 people at most.
    for (var people = 3; people <= 8; people++) {
      final step = cameraStep(people);
      expect(step.kbps, lessThanOrEqualTo(last.kbps));
      expect(step.lines, lessThanOrEqualTo(last.lines ?? 1080));
      expect(step.fps, lessThanOrEqualTo(last.fps));
      expect((people - 1) * step.kbps, lessThanOrEqualTo(3000));
      last = step;
    }
    expect(cameraStep(3), cameraStep(4));
    expect(cameraStep(5), cameraStep(6));
    expect(cameraStep(7), cameraStep(8));
    expect(cameraStep(8).lines, lessThanOrEqualTo(360));
  });

  test('the picture is scaled down from what the camera gives', () {
    final step = cameraStep(6);
    expect(cameraScale(720, step), 2);
    expect(cameraScale(1080, step), 3);
    expect(cameraScale(480, step), closeTo(1.33, 0.01));
    // Smaller than the step already: as it is.
    expect(cameraScale(240, step), 1);
    // Before the first picture: the 720 lines asked for.
    expect(cameraScale(0, step), 2);
  });

  test('people coming and going step the camera on every connection', () async {
    final call = Call(group: 'g', me: 'me');
    final connection = _Connection();
    call.peers['ana'] = CallPeer('ana')..connection = connection;
    RTCRtpEncoding? camera() => connection.channels[1].set;
    RTCRtpEncoding? screen() => connection.channels[2].set;

    Future<void> people(int n) async {
      await call.setParticipants([
        'me',
        'ana',
        for (var i = 2; i < n; i++) 'p$i',
      ]);
      // The connections are told after the room's change is taken.
      await pumpEventQueue();
    }

    await people(8);
    expect(camera()!.maxBitrate, 350000);
    expect(camera()!.scaleResolutionDownBy, 2);
    expect(camera()!.maxFramerate, 20);
    // The screen keeps its own limits.
    expect(screen()!.maxBitrate, screenKbps(1920, 1080, 30) * 1000);
    expect(screen()!.maxFramerate, isNot(20));

    await people(4);
    expect(camera()!.maxBitrate, 900000);
    expect(camera()!.scaleResolutionDownBy, closeTo(1.33, 0.01));

    await people(2);
    expect(camera()!.maxBitrate, cameraMaxKbps * 1000);
    expect(camera()!.scaleResolutionDownBy, 1);
    expect(camera()!.maxFramerate, 60);

    // Nothing changes, nothing is sent again.
    connection.channels[1].set = null;
    await people(2);
    expect(camera(), isNull);
  });

  test(
    'a screen rate chosen before sharing reaches every connection',
    () async {
      final call = Call(group: 'g', me: 'me');
      final connection = _Connection();
      call.peers['ana'] = CallPeer('ana')..connection = connection;
      RTCRtpEncoding? screen() => connection.channels[2].set;

      await call.setScreenQuality(const ScreenQuality(fps: 60));
      expect(screen()!.maxFramerate, 60);
      // A faster screen may use more: before its size is known, as a full 1080p one.
      expect(screen()!.maxBitrate, 12442000);

      await call.setScreenQuality(const ScreenQuality(fps: 15));
      expect(screen()!.maxFramerate, 15);
      expect(screen()!.maxBitrate, screenLeastKbps * 1000);
    },
  );

  test('a screen may use more the larger and faster it goes out', () {
    // 0.1 bit for each pixel of each frame: 12.4 Mbit/s for 1080p at 60.
    expect(screenKbps(1920, 1080, 60), 12442);
    // Never under what every screen had before, nor over the most any may use.
    expect(screenKbps(1280, 720, 30), screenLeastKbps);
    expect(screenKbps(1920, 1080, 15), screenLeastKbps);
    expect(screenKbps(3840, 2160, 60), screenMaxKbps);
    expect(screenKbps(0, 0, 60), screenLeastKbps);
  });
}
