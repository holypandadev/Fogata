import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// A connection whose four channels note what they are given, with [stats] as its
/// statistics.
class _Connection implements RTCPeerConnection {
  _Connection([MediaStreamTrack? screenIn])
    : channels = [for (var i = 0; i < 4; i++) _Transceiver(i, screenIn)];
  final List<_Transceiver> channels;
  List<StatsReport> stats = [];

  @override
  Function(RTCPeerConnectionState state)? onConnectionState;
  @override
  Function(RTCTrackEvent event)? onTrack;
  @override
  Function(RTCIceGatheringState state)? onIceGatheringState;
  @override
  Function(RTCIceConnectionState state)? onIceConnectionState;

  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async => channels;

  @override
  Future<List<StatsReport>> getStats([MediaStreamTrack? track]) async => stats;

  @override
  Future<void> close() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transceiver implements RTCRtpTransceiver {
  _Transceiver(int index, MediaStreamTrack? received)
    : mid = '$index',
      receiver = _Receiver(index == 2 ? received : null);
  final _sender = _Sender();

  @override
  final String mid;

  @override
  RTCRtpSender get sender => _sender;

  @override
  final RTCRtpReceiver receiver;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Sender implements RTCRtpSender {
  /// The ids of the tracks given, null for none.
  final given = <String?>[];

  @override
  Future<void> replaceTrack(MediaStreamTrack? track) async =>
      given.add(track?.id);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Receiver implements RTCRtpReceiver {
  _Receiver(this.track);

  @override
  final MediaStreamTrack? track;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Track implements MediaStreamTrack {
  _Track(this.id);

  @override
  final String id;

  @override
  String? get kind => 'video';

  @override
  Future<void> stop() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// This device's shared screen, without sound.
class _Screen implements MediaStream {
  final track = _Track('screen');

  @override
  String get id => 'shared';

  @override
  List<MediaStreamTrack> getVideoTracks() => [track];

  @override
  List<MediaStreamTrack> getAudioTracks() => [];

  @override
  List<MediaStreamTrack> getTracks() => [track];

  @override
  Future<void> dispose() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

StatsReport _screenOut(num frames) => StatsReport('out-2', 'outbound-rtp', 0, {
  'kind': 'video',
  'mid': '2',
  'framesEncoded': frames,
});

const me = '0000000000000000';
const ana = 'a000000000000000';
const zoe = 'f000000000000000';

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  // What the platform was asked, in order, with what it was given.
  final asked = <(String, Object?)>[];
  final events = <int, MockStreamHandlerEventSink>{};
  final sent = <(String, String)>[];
  var textures = 0;
  var streams = 0;

  setUp(() {
    asked.clear();
    events.clear();
    sent.clear();
    textures = 0;
    streams = 0;
    signalsForTests = (to, message) => sent.add((to, message['t'] as String));
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        asked.add((call.method, call.arguments));
        switch (call.method) {
          case 'fogataScreenRefresh':
            return true;
          case 'createVideoRenderer':
            final id = ++textures;
            messenger.setMockStreamHandler(
              EventChannel('FlutterWebRTC/Texture$id'),
              MockStreamHandler.inline(
                onListen: (arguments, sink) => events[id] = sink,
              ),
            );
            return <String, dynamic>{'textureId': id};
          case 'createLocalMediaStream':
            return <String, dynamic>{'streamId': 'made-${++streams}'};
        }
        return <String, dynamic>{};
      },
    );
  });
  tearDown(() {
    signalsForTests = null;
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  /// The tracks the platform was asked to give the last picture of again.
  List<Object?> refreshed() => [
    for (final (method, arguments) in asked)
      if (method == 'fogataScreenRefresh') (arguments as Map)['trackId'],
  ];

  /// This device shares its screen; Ana is connected and does not watch it yet.
  (Call, CallPeer, _Connection) sharing() {
    final call = Call(group: 'g', me: me, mic: false)..sharing = true;
    call.holdForTests(streams: [null, null, _Screen()]);
    final connection = _Connection();
    final peer = CallPeer(ana)
      ..connection = connection
      ..connected = true;
    call.peers[ana] = peer;
    return (call, peer, connection);
  }

  Map<String, dynamic> state(String watch) => {
    't': 'state',
    'mic': true,
    'cam': false,
    'screen': false,
    'sound': false,
    'watch': watch,
  };

  group('the sharer', () {
    test('gives the capture\'s last picture again to someone who starts '
        'watching', () async {
      final (call, _, connection) = sharing();
      await call.handleSignal(ana, state(me));
      // The screen goes to her, and her encoder gets a picture even if the screen stays
      // still, as a portal's or ScreenCaptureKit's capture gives none then.
      expect(connection.channels[2]._sender.given, ['screen']);
      expect(refreshed(), ['screen']);
      // Someone who stops watching gets nothing more.
      await call.handleSignal(ana, state(''));
      expect(connection.channels[2]._sender.given, ['screen', null]);
      expect(refreshed(), ['screen']);
      await call.stop();
    });

    test(
      'sends the screen again, a few times, to a watcher to whom nothing has '
      'gone out',
      () async {
        final (call, peer, connection) = sharing();
        final screen = connection.channels[2]._sender;
        connection.stats = [_screenOut(0)];
        peer.watches = me;
        Future<void> reading() async {
          await call.measureNow();
          await pumpEventQueue();
        }

        // The first reading that finds her watching only takes note.
        await reading();
        expect(refreshed(), isEmpty);
        // Nothing went out by the next: the capture's last picture again.
        await reading();
        expect(refreshed(), ['screen']);
        expect(screen.given, isEmpty);
        // Still nothing: the screen is also put on her connection again, as her closing
        // and opening it would.
        await reading();
        expect(refreshed(), ['screen', 'screen']);
        expect(screen.given, [null, 'screen']);
        await reading();
        expect(refreshed(), hasLength(Call.screenNudges));
        // Never more than that.
        for (var i = 0; i < 4; i++) {
          await reading();
        }
        expect(refreshed(), hasLength(Call.screenNudges));
        expect(screen.given, [null, 'screen', null, 'screen']);
        await call.stop();
      },
    );

    test('leaves alone a watcher to whom the screen went out', () async {
      final (call, peer, connection) = sharing();
      // Her encoder had made 40 pictures when she last watched.
      connection.stats = [_screenOut(40)];
      await call.measureNow();
      peer.watches = me;
      await call.measureNow();
      connection.stats = [_screenOut(41)];
      for (var i = 0; i < 4; i++) {
        await call.measureNow();
        await pumpEventQueue();
      }
      expect(refreshed(), isEmpty);
      // Watching again after she stopped, to whom nothing goes out this time: asked
      // again, from the start.
      peer.watches = null;
      await call.measureNow();
      peer.watches = me;
      for (var i = 0; i < 2; i++) {
        await call.measureNow();
        await pumpEventQueue();
      }
      expect(refreshed(), ['screen']);
      // A new connection starts over too.
      final made = _Connection()..stats = [_screenOut(0)];
      peer.connection = made;
      for (var i = 0; i < 2; i++) {
        await call.measureNow();
        await pumpEventQueue();
      }
      expect(refreshed(), ['screen', 'screen']);
      await call.stop();
    });
  });

  group('the watcher', () {
    /// Ana shares her screen and offers this device a connection, whose screen arrives
    /// without a stream of its own, as every copy sends it.
    Future<(Call, CallPeer, _Connection)> watching() async {
      final call = Call(group: 'g', me: zoe);
      final track = _Track('her-screen');
      final connection = _Connection(track);
      final peer = CallPeer(ana)..sharing = true;
      call.peers[ana] = peer;
      call.connectionsForTests = () async => connection;
      try {
        await call.handleSignal(ana, {
          't': 'offer',
          'sdp':
              'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 opus/48000/2\r\na=mid:0\r\n'
              'm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\na=mid:1\r\n'
              'm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 VP8/90000\r\na=mid:2\r\n'
              'm=audio 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 opus/48000/2\r\na=mid:3\r\n',
          'n': 1,
        });
      } catch (_) {
        // The fake goes no further than making the connection.
      }
      connection.onTrack!(
        RTCTrackEvent(
          streams: [],
          track: track,
          transceiver: connection.channels[2],
        ),
      );
      await Future<void>.delayed(const Duration(milliseconds: 50));
      expect(peer.screen.srcObject?.id, 'made-1');
      peer.connected = true;
      call.watch(ana);
      return (call, peer, connection);
    }

    /// The streams the screen's renderer was given, in order ('' for none).
    List<Object?> shownOnScreen(CallPeer peer) => [
      for (final (method, arguments) in asked)
        if (method == 'videoRendererSetSrcObject' &&
            (arguments as Map)['textureId'] == peer.screen.textureId)
          arguments['streamId'],
    ];

    int told() => sent.where((s) => s == (ana, 'state')).length;

    test('shows a screen that has no picture yet again, and tells its sharer '
        'again, a few times', () async {
      final (call, peer, _) = await watching();
      expect(shownOnScreen(peer), ['made-1']);
      expect(told(), 1);
      // How many times Ana was told, and the renderer given a stream, by each reading.
      final after = <int, (int, int)>{};
      for (var reading = 1; reading <= 12; reading++) {
        await call.measureNow();
        await pumpEventQueue();
        after[reading] = (told(), shownOnScreen(peer).length);
      }
      // At the second, fourth and eighth readings: the renderer starts over with the
      // same stream (given none first), and Ana hears again that this device watches
      // her screen.
      expect(after[1], (1, 1));
      expect(after[2], (2, 3));
      expect(after[3], (2, 3));
      expect(after[4], (3, 5));
      expect(after[7], (3, 5));
      expect(after[8], (4, 7));
      expect(after[12], (4, 7));
      expect(shownOnScreen(peer), [
        'made-1',
        for (var i = 0; i < 3; i++) ...['', 'made-1'],
      ]);
      await call.stop();
    });

    test('leaves a screen that shows its picture alone', () async {
      final (call, peer, _) = await watching();
      events[peer.screen.textureId!]!.success({
        'event': 'didTextureChangeVideoSize',
        'id': peer.screen.textureId,
        'width': 1920,
        'height': 1080,
      });
      await pumpEventQueue();
      for (var reading = 1; reading <= 4; reading++) {
        await call.measureNow();
        await pumpEventQueue();
      }
      expect(shownOnScreen(peer), ['made-1']);
      expect(told(), 1);
      await call.stop();
    });

    test('shows the track the connection receives now, in a stream of its own, '
        'when the renderer shows another', () async {
      final (call, peer, connection) = await watching();
      // A track that is not the one the connection receives on its screen's channel now.
      peer.screen.srcObject = await createLocalMediaStream('other');
      await pumpEventQueue();
      for (var reading = 1; reading <= 2; reading++) {
        await call.measureNow();
        await pumpEventQueue();
      }
      expect(shownOnScreen(peer).last, 'made-3');
      expect(peer.screen.srcObject?.getVideoTracks().single.id, 'her-screen');
      expect(connection.channels[2].receiver.track?.id, 'her-screen');
      await call.stop();
    });
  });
}
