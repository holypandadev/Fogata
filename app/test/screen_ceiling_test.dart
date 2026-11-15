import 'package:fake_async/fake_async.dart';
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

  /// How many times it was told how to send.
  int sets = 0;

  @override
  RTCRtpParameters get parameters =>
      RTCRtpParameters(encodings: [RTCRtpEncoding()]);

  @override
  Future<bool> setParameters(RTCRtpParameters parameters) async {
    set = parameters.encodings!.single;
    sets++;
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

/// A call sharing a [width] by [height] screen at [quality] (as captured, unless said), as
/// where its watchers share one encoder of it: their number then changes no level, only
/// the ceiling. Six others are in the call, their connections up; none watches yet.
class _Sharing {
  _Sharing(
    this.time,
    double width,
    double height, {
    ScreenQuality quality = const ScreenQuality(lines: null),
  }) {
    call.screenLoad.sharedEncode = true;
    call.screenQuality = quality;
    call.localScreen.value = RTCVideoValue.empty.copyWith(
      width: width,
      height: height,
    );
    call.rescaleNow();
    time.flushMicrotasks();
    for (var i = 0; i < 6; i++) {
      final id = 'p$i';
      final connection = connections[id] = _Connection();
      call.peers[id] = CallPeer(id)
        ..connection = connection
        ..connected = true;
    }
  }

  final FakeAsync time;
  final call = Call(group: 'g', me: 'me')..sharing = true;
  final connections = <String, _Connection>{};

  _Sender sender(String id) => connections[id]!.channels[2];

  int? limitTo(String id) => sender(id).set?.maxBitrate;

  /// [id] says it starts (or stops) watching, and what that tunes is done.
  void watch(String id, {bool on = true}) {
    call.handleSignal(id, {'t': 'state', 'watch': on ? 'me' : ''});
    time.flushMicrotasks();
  }

  /// A reading of the statistics, as every 2 s.
  void read() {
    call.measureNow();
    time.flushMicrotasks();
  }

  void expectAll(List<String> ids, int bitrate) {
    for (final id in ids) {
      expect(limitTo(id), bitrate, reason: id);
    }
  }
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

  test('from three watchers, the less each one\'s stream may use', () {
    // One or two watchers: what the size and rate allow, as before.
    for (final watchers in [0, 1, 2]) {
      expect(screenShareKbps(6000, watchers), 6000);
      // 1440 lines at 30 frames a second, and 1080 at 60.
      expect(screenShareKbps(11059, watchers), 11059);
      expect(screenShareKbps(12442, watchers), 12442);
      expect(screenShareKbps(screenMaxKbps, watchers), screenMaxKbps);
    }
    // From three: 12 Mbit/s divided among them.
    expect(screenShareKbps(6000, 3), 4000);
    expect(screenShareKbps(12442, 3), 4000);
    expect(screenShareKbps(6000, 4), 3000);
    expect(screenShareKbps(6000, 5), 2400);
    expect(screenShareKbps(6000, 6), 2000);
    expect(screenShareKbps(6000, 7), 1714);
    expect(screenShareKbps(6000, 8), 1500);
    // Never more than the size and rate allow.
    expect(screenShareKbps(1000, 3), 1000);
    for (var n = 3; n <= 7; n++) {
      // Three or more never send more than the budget together...
      expect(n * screenShareKbps(screenMaxKbps, n), lessThanOrEqualTo(12000));
      // ...and each gets the same at any size and rate, so those sent one size keep
      // sharing its encoder.
      expect(
        screenShareKbps(screenKbps(960, 540, 5), n),
        screenShareKbps(screenKbps(3840, 2160, 60), n),
      );
    }
  });

  test('each stream keeps a usable least however many watch', () {
    expect(screenShareKbps(6000, 9), screenLeastShareKbps);
    expect(screenShareKbps(screenMaxKbps, 20), screenLeastShareKbps);
    expect(screenShareKbps(screenMaxKbps, 1000), screenLeastShareKbps);
    // A call of eight has seven watchers at most: they stay over it.
    expect(screenShareKbps(6000, 7), greaterThan(screenLeastShareKbps));
  });

  test(
    'watchers coming and going change the limit of those already watching',
    () => fakeAsync((time) {
      // A 1440-line screen shared at its own size.
      final s = _Sharing(time, 2560, 1440);
      // One or two: what its size and rate allow, as before.
      s.watch('p0');
      expect(s.limitTo('p0'), screenKbps(2560, 1440, 30) * 1000);
      s.watch('p1');
      expect(s.limitTo('p0'), 11059000);
      expect(s.limitTo('p1'), 11059000);

      // Three and four: 1080 lines from three, and 4, then 3 Mbit/s each, the others'
      // at once (the fourth changes neither the ceiling nor the level, only the limit).
      s.watch('p2');
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
      time.elapse(Call.sharesEvery);
      s.watch('p3');
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);
      // Who does not watch is told nothing.
      expect(s.limitTo('p5'), isNull);

      // Five: 720 lines, 2.4 Mbit/s each.
      time.elapse(Call.sharesEvery);
      s.watch('p4');
      s.expectAll(['p0', 'p1', 'p2', 'p3', 'p4'], 2400000);

      // They go: the others get their share back at once, though the ceiling waits.
      time.elapse(Call.sharesEvery);
      s.watch('p4', on: false);
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);
      time.elapse(Call.sharesEvery);
      s.watch('p3', on: false);
      expect(s.call.screenLoad.ceiling, 720);
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
      time.elapse(Call.sharesEvery);
      s.watch('p2', on: false);
      // Two, at the 720 lines the ceiling keeps for now: what they allow.
      s.expectAll(['p0', 'p1'], screenLeastKbps * 1000);
    }),
  );

  test('two watchers of a screen at 60 frames a second keep its limit', () {
    fakeAsync((time) {
      final s = _Sharing(
        time,
        1920,
        1080,
        quality: const ScreenQuality(lines: null, fps: 60),
      );
      s.watch('p0');
      s.watch('p1');
      s.expectAll(['p0', 'p1'], 12442000);
      expect(s.sender('p0').set!.maxFramerate, 60);
      s.watch('p2');
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
    });
  });

  test(
    'someone watching on and off again and again is followed once a second at most',
    () => fakeAsync((time) {
      final s = _Sharing(time, 1920, 1080);
      for (final id in ['p0', 'p1', 'p2']) {
        s.watch(id);
      }
      time.elapse(Call.sharesEvery);
      final sent = s.sender('p0').sets;
      for (var i = 0; i < 10; i++) {
        s.watch('p3');
        s.watch('p3', on: false);
      }
      // Told at once the first time, and the rest waits.
      expect(s.sender('p0').sets, sent + 1);
      time.elapse(Call.sharesEvery);
      // Then once, with the number there is: three.
      expect(s.sender('p0').sets, sent + 2);
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
      time.elapse(Call.sharesEvery * 5);
      expect(s.sender('p0').sets, sent + 2);
    }),
  );

  test(
    'a watcher who came while the limit kept quiet is told the one there is after',
    () => fakeAsync((time) {
      final s = _Sharing(time, 1920, 1080);
      for (final id in ['p0', 'p1', 'p2', 'p3']) {
        s.watch(id);
      }
      time.elapse(Call.sharesEvery);
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);
      // One goes: 4 Mbit/s each at once.
      s.watch('p3', on: false);
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
      // Within the second after: one comes, told 3 as four watch, and one goes.
      s.watch('p4');
      expect(s.limitTo('p4'), 3000000);
      s.watch('p2', on: false);
      // Three watch again, as when the others were told 4: the one who came is told it
      // too.
      time.elapse(Call.sharesEvery);
      s.expectAll(['p0', 'p1', 'p4'], 4000000);
    }),
  );

  test(
    'only watchers whose connection is up divide the budget, and one gone from the '
    'call is counted out at the next reading',
    () => fakeAsync((time) {
      final s = _Sharing(time, 1920, 1080);
      for (final id in ['p0', 'p1', 'p2', 'p3']) {
        s.watch(id);
      }
      time.elapse(Call.sharesEvery);
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);

      // p3's connection fails: while it is made again, the others share among three.
      s.call.peers['p3']!.connected = false;
      s.read();
      s.expectAll(['p0', 'p1', 'p2'], 4000000);
      // It still watches, and the size's ceiling still counts it.
      expect(s.call.watchers, 4);
      expect(s.call.screenLoad.ceiling, 1080);
      // Up again: four.
      time.elapse(Call.sharesEvery);
      s.call.peers['p3']!.connected = true;
      s.read();
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);

      // Kept out: its connection closed, and none made while it stays.
      time.elapse(Call.sharesEvery);
      s.call.peers['p3']!
        ..connection = null
        ..connected = false;
      s.read();
      s.expectAll(['p0', 'p1', 'p2'], 4000000);

      // p2 leaves the call without saying it stopped watching: two are left, at what
      // the screen allows.
      time.elapse(Call.sharesEvery);
      s.call.peers.remove('p2');
      s.read();
      s.expectAll(['p0', 'p1'], screenKbps(1920, 1080, 30) * 1000);
    }),
  );

  test(
    'another quality chosen while they watch tells each connection once',
    () => fakeAsync((time) {
      final s = _Sharing(time, 2560, 1440);
      s.watch('p0');
      s.watch('p1');
      time.elapse(Call.sharesEvery);
      s.expectAll(['p0', 'p1'], 11059000);
      s.call.setScreenQuality(const ScreenQuality(lines: 720, fps: 60));
      time.flushMicrotasks();
      final sent = s.sender('p0').sets;
      s.expectAll(['p0', 'p1'], screenLeastKbps * 1000);
      expect(s.sender('p0').set!.maxFramerate, 60);
      // The next reading finds nothing changed.
      s.read();
      time.elapse(Call.sharesEvery);
      expect(s.sender('p0').sets, sent);
    }),
  );

  test(
    'a watcher whose connection steps the screen down gets the limit of the others',
    () => fakeAsync((time) {
      final s = _Sharing(time, 1920, 1080);
      for (final id in ['p0', 'p1', 'p2']) {
        s.watch(id);
      }
      // p1's connection falls short: it gets the screen a step smaller, 720 lines.
      s.call.peers['p1']!.screenFit.step = 1;
      time.elapse(Call.sharesEvery);
      s.watch('p3');
      expect(s.sender('p0').set!.scaleResolutionDownBy, 1);
      expect(s.sender('p1').set!.scaleResolutionDownBy, 1.5);
      s.expectAll(['p0', 'p1', 'p2', 'p3'], 3000000);
    }),
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
