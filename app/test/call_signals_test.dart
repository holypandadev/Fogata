import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/quality.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'wait.dart';

/// A connection that keeps the descriptions set on it, with [stats] as its statistics.
class _Connection implements RTCPeerConnection {
  final remote = <String>[];
  List<StatsReport> stats = [];
  bool closed = false;

  @override
  Function(RTCPeerConnectionState state)? onConnectionState;

  @override
  Function(RTCTrackEvent event)? onTrack;

  @override
  Function(RTCIceGatheringState state)? onIceGatheringState;

  @override
  Function(RTCIceConnectionState state)? onIceConnectionState;

  @override
  Future<void> setRemoteDescription(RTCSessionDescription description) async =>
      remote.add(description.sdp ?? '');

  @override
  Future<void> addCandidate(RTCIceCandidate candidate) async {}

  @override
  Future<List<StatsReport>> getStats([MediaStreamTrack? track]) async => stats;

  @override
  Future<void> close() async => closed = true;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

// Device ids are hex, as long as the logs shorten them to.
const me = '0000000000000000';
const ana = 'a000000000000000';
const bea = 'b000000000000000';
const zoe = 'f000000000000000';

/// A description with a section of each kind, named by its place, in the codec a call
/// keeps for that kind, or in [video] for the video sections.
String described(List<String> kinds, {String video = 'VP8'}) => [
  'v=0',
  for (final (i, kind) in kinds.indexed) ...[
    'm=$kind 9 UDP/TLS/RTP/SAVPF 96',
    kind == 'audio' ? 'a=rtpmap:96 opus/48000/2' : 'a=rtpmap:96 $video/90000',
    'a=mid:$i',
  ],
  '',
].join('\r\n');

final callKinds = ['audio', 'video', 'video', 'audio'];

void main() {
  test('an answer to an earlier offer is dropped', () async {
    final call = Call(group: 'g', me: me);
    final connection = _Connection();
    call.peers[ana] = CallPeer(ana)..connection = connection;
    final latest = call.offerNumberTo(ana)!;
    await call.handleSignal(ana, {
      't': 'answer',
      'sdp': 'old',
      'n': latest - 1,
    });
    expect(connection.remote, isEmpty);
    await call.handleSignal(ana, {'t': 'answer', 'sdp': 'new', 'n': latest});
    // From a copy that names no offer, as before.
    await call.handleSignal(ana, {'t': 'answer', 'sdp': 'older copy'});
    expect(connection.remote, ['new', 'older copy']);
  });

  Map<String, dynamic> state({bool screen = false, bool cam = false}) => {
    't': 'state',
    'mic': true,
    'cam': cam,
    'screen': screen,
    'sound': false,
    'watch': '',
  };

  test('a watched screen is shown again when its sharer comes back', () async {
    final call = Call(group: 'g', me: me);
    call.peers[ana] = CallPeer(ana)..sharing = true;
    call.watch(ana);
    // Her network was gone a while: she left the room and came back.
    await call.setParticipants([me]);
    expect(call.watching, isNull);
    call.peers[ana] = CallPeer(ana);
    await call.handleSignal(ana, state(screen: true));
    expect(call.watching, ana);
  });

  test('nothing comes back once something else is shown', () async {
    final call = Call(group: 'g', me: me);
    call.peers[ana] = CallPeer(ana)..sharing = true;
    call.peers[bea] = CallPeer(bea)..sharing = true;
    call.watch(ana);
    await call.setParticipants([me, bea]);
    call.watch(bea);
    call.peers[ana] = CallPeer(ana);
    await call.handleSignal(ana, state(screen: true));
    expect(call.watching, bea);
  });

  test('a camera that filled the room fills it again', () async {
    final call = Call(group: 'g', me: me);
    call.peers[ana] = CallPeer(ana)..cam = true;
    call.focus(ana);
    await call.setParticipants([me]);
    expect(call.focused, isNull);
    call.peers[ana] = CallPeer(ana);
    await call.handleSignal(ana, state(cam: true));
    expect(call.focused, ana);
  });

  test('a camera that is off when she comes back is not shown', () async {
    final call = Call(group: 'g', me: me);
    call.peers[ana] = CallPeer(ana)..cam = true;
    call.focus(ana);
    await call.setParticipants([me]);
    call.peers[ana] = CallPeer(ana);
    await call.handleSignal(ana, state());
    expect(call.focused, isNull);
  });

  test('a description takes no lowest bitrate and a start bitrate no higher '
      'than copies ask for; one as copies send it stays as it is', () async {
    String withFmtp(String fmtp) =>
        described(callKinds)
            .replaceFirst('a=mid:1\r\n', 'a=fmtp:96 $fmtp\r\na=mid:1\r\n');
    // As every copy describes it.
    final honest = withFmtp(
      'x-google-max-bitrate=$cameraMaxKbps;'
      'x-google-start-bitrate=$cameraStartKbps',
    );
    // This device offers, Ana answers.
    final call = Call(group: 'g', me: me);
    final connection = _Connection();
    call.peers[ana] = CallPeer(ana)..connection = connection;
    final latest = call.offerNumberTo(ana)!;
    for (final sdp in [
      honest,
      withFmtp(
        'x-google-min-bitrate=20000;x-google-max-bitrate=20000;'
        'x-google-start-bitrate=20000',
      ),
      withFmtp('x-google-min-bitrate=20000'),
    ]) {
      await call.handleSignal(ana, {'t': 'answer', 'sdp': sdp, 'n': latest});
    }
    expect(connection.remote, [
      honest,
      withFmtp(
        'x-google-max-bitrate=20000;x-google-start-bitrate=$maxStartKbps',
      ),
      described(callKinds),
    ]);
    // Zoe answers Ana, who offers, on a new connection.
    final answering = Call(group: 'g', me: zoe);
    final made = _Connection();
    answering.connectionsForTests = () async => made;
    answering.peers[ana] = CallPeer(ana);
    try {
      await answering.handleSignal(ana, {
        't': 'offer',
        'sdp': withFmtp('x-google-min-bitrate=5000;x-google-max-bitrate=1500'),
        'n': 1,
      });
    } catch (_) {
      // The fake goes no further than taking the offer.
    }
    expect(made.remote, [withFmtp('x-google-max-bitrate=1500')]);
    await call.stop();
    await answering.stop();
  });

  test('an offer from the side that does not offer is dropped', () async {
    // This device has the smaller id: it offers, Ana answers.
    final call = Call(group: 'g', me: me);
    final connection = _Connection();
    final peer = CallPeer(ana)..connection = connection;
    call.peers[ana] = peer;
    for (final update in [false, true]) {
      await call.handleSignal(ana, {
        't': 'offer',
        'sdp': described(callKinds),
        'update': update,
        'n': 1,
      });
    }
    expect(connection.remote, isEmpty);
    expect(connection.closed, isFalse);
    expect(peer.connection, connection);
  });

  test('an offer with channels a call does not have is dropped', () async {
    // Ana has the smaller id and offers, but more than the call's channels.
    final call = Call(group: 'g', me: zoe);
    final connection = _Connection();
    final peer = CallPeer(ana)..connection = connection;
    call.peers[ana] = peer;
    for (final kinds in [
      [...callKinds, 'video', 'video', 'video', 'video'],
      ['video', 'video', 'video', 'video'],
    ]) {
      for (final update in [false, true]) {
        await call.handleSignal(ana, {
          't': 'offer',
          'sdp': described(kinds),
          'update': update,
          'n': 1,
        });
      }
    }
    expect(connection.remote, isEmpty);
    expect(connection.closed, isFalse);
    expect(peer.connection, connection);
  });

  test(
    'someone sending pictures larger than any copy sends is kept out',
    () async {
      final call = Call(
        group: 'g',
        me: zoe,
        nameOf: (id) => id == ana ? 'Ana' : 'Bea',
      );
      final told = <String>[];
      call.onProblem = told.add;
      final connection = _Connection()
        ..stats = [
          StatsReport('in-1', 'inbound-rtp', 0, {
            'kind': 'video',
            'mid': '1',
            'frameWidth': 16383,
            'frameHeight': 16383,
          }),
        ];
      final peer = CallPeer(ana)
        ..connection = connection
        ..connected = true;
      call.peers[ana] = peer;
      // Heard from before, as a connection that came up is.
      call.quality.take(
        ana,
        LinkSample.of(const [], voiceMid: '0'),
        DateTime.now(),
      );
      expect(call.quality.linkOf(ana), isNotNull);
      await call.measureNow();
      expect(peer.keptOut, isTrue);
      // Said in the call, once; not shown as reconnecting.
      expect(told, ['Lost the connection with Ana.']);
      expect(call.quality.linkOf(ana), isNull);
      expect(connection.closed, isTrue);
      expect(peer.connection, isNull);
      // No new connection either, whoever asks for it.
      await expectLater(
        call.handleSignal(ana, {
          't': 'offer',
          'sdp': described(callKinds),
          'n': 2,
        }),
        throwsStateError,
      );
      expect(peer.connection, isNull);
      // Pictures of a size a copy sends keep the connection.
      final fine = _Connection()
        ..stats = [
          StatsReport('in-2', 'inbound-rtp', 0, {
            'kind': 'video',
            'mid': '2',
            'frameWidth': 3840,
            'frameHeight': 2160,
          }),
        ];
      final other = CallPeer(bea)
        ..connection = fine
        ..connected = true;
      call.peers[bea] = other;
      await call.measureNow();
      expect(other.keptOut, isFalse);
      expect(told, hasLength(1));
      expect(fine.closed, isFalse);
    },
  );

  test(
    'someone sending pictures far faster than any copy sends is kept out',
    () async {
      final call = Call(
        group: 'g',
        me: zoe,
        nameOf: (id) => id == ana ? 'Ana' : 'Bea',
      );
      final told = <String>[];
      call.onProblem = told.add;
      StatsReport camera(num frames, num bytes) =>
          StatsReport('in-1', 'inbound-rtp', 0, {
            'kind': 'video',
            'mid': '1',
            'frameWidth': 1280,
            'frameHeight': 720,
            'framesDecoded': frames,
            'bytesReceived': bytes,
          });
      final flooding = _Connection()..stats = [camera(0, 0)];
      final peer = CallPeer(ana)
        ..connection = flooding
        ..connected = true;
      call.peers[ana] = peer;
      final fine = _Connection()..stats = [camera(0, 0)];
      final other = CallPeer(bea)
        ..connection = fine
        ..connected = true;
      call.peers[bea] = other;
      // Readings a second apart: Ana's camera at about 500 frames a second, Bea's at 60
      // and 1500 kbit/s, a copy's most.
      Future<void> reading(int i) async {
        flooding.stats = [camera(500 * i, 150000 * i)];
        fine.stats = [camera(60 * i, 187500 * i)];
        await call.measureNow();
        await Future<void>.delayed(const Duration(seconds: 1));
      }

      await reading(0);
      // The first reading above the limit is let through.
      await reading(1);
      expect(peer.keptOut, isFalse);
      await reading(2);
      expect(peer.keptOut, isTrue);
      expect(flooding.closed, isTrue);
      expect(told, ['Lost the connection with Ana.']);
      expect(other.keptOut, isFalse);
      expect(fine.closed, isFalse);
    },
  );

  test(
    'a picture larger than any copy sends keeps its sender out as it arrives',
    () async {
      final call = Call(
        group: 'g',
        me: zoe,
        nameOf: (id) => id == ana ? 'Ana' : 'Bea',
      );
      final told = <String>[];
      call.onProblem = told.add;
      // Ana's connection stopped answering a moment ago: not connected, though her
      // pictures may still come.
      final connection = _Connection();
      final peer = CallPeer(ana)..connection = connection;
      call.peers[ana] = peer;
      final fine = _Connection();
      final other = CallPeer(bea)
        ..connection = fine
        ..connected = true;
      call.peers[bea] = other;
      // Pictures of the sizes copies send: a camera in Full HD, a screen in 4K.
      call.pictureArrived(ana, 1, 1920, 1080);
      call.pictureArrived(bea, 2, 3840, 2160);
      await pumpEventQueue();
      expect(peer.keptOut, isFalse);
      expect(other.keptOut, isFalse);
      // One the renderer dropped as too large to draw, between two of a fair size.
      call.pictureArrived(ana, 1, 16382, 16382);
      await pumpEventQueue();
      expect(peer.keptOut, isTrue);
      expect(connection.closed, isTrue);
      expect(peer.connection, isNull);
      expect(told, ['Lost the connection with Ana.']);
      // A camera larger than Full HD, though drawn, is no copy's either.
      call.pictureArrived(bea, 1, 2560, 1440);
      await pumpEventQueue();
      expect(other.keptOut, isTrue);
      expect(fine.closed, isTrue);
      expect(told, hasLength(2));
    },
  );

  test(
    'a connection made while its participant is kept out is closed',
    () async {
      // Ana has the smaller id and offers; this device answers on a new connection.
      final call = Call(group: 'g', me: zoe);
      final made = Completer<RTCPeerConnection>();
      call.connectionsForTests = () => made.future;
      final peer = CallPeer(ana);
      call.peers[ana] = peer;
      final answering = call.handleSignal(ana, {
        't': 'offer',
        'sdp': described(callKinds),
        'n': 1,
      });
      await pumpEventQueue();
      // While WebRTC makes the connection, a picture of hers keeps her out.
      call.pictureArrived(ana, 2, 16382, 16382);
      await pumpEventQueue();
      expect(peer.keptOut, isTrue);
      final connection = _Connection();
      made.complete(connection);
      await expectLater(answering, throwsStateError);
      expect(connection.closed, isTrue);
      expect(peer.connection, isNull);
    },
  );

  test(
    'a failure of a connection that was replaced is not the live one\'s',
    () async {
      // Ana has the smaller id and offers; this device answers on a new connection, twice.
      final call = Call(group: 'g', me: zoe);
      final made = <_Connection>[];
      call.connectionsForTests = () async {
        final connection = _Connection();
        made.add(connection);
        return connection;
      };
      final peer = CallPeer(ana);
      call.peers[ana] = peer;
      Future<void> offered(int n) async {
        try {
          await call.handleSignal(ana, {
            't': 'offer',
            'sdp': described(callKinds),
            'n': n,
          });
        } catch (_) {
          // The fake goes no further than making the connection.
        }
      }

      await offered(1);
      await offered(2);
      expect(made, hasLength(2));
      final (old, live) = (made[0], made[1]);
      expect(peer.connection, live);
      live.onConnectionState!(
        RTCPeerConnectionState.RTCPeerConnectionStateConnected,
      );
      expect(peer.connected, isTrue);
      // The platform tears the replaced one down and its channel reports a failure late.
      old.onConnectionState!(
        RTCPeerConnectionState.RTCPeerConnectionStateFailed,
      );
      expect(peer.connected, isTrue);
      expect(
        peer.state,
        RTCPeerConnectionState.RTCPeerConnectionStateConnected,
      );
      // No new try is made: the live connection stays.
      await Future<void>.delayed(const Duration(seconds: 5));
      expect(made, hasLength(2));
      expect(peer.connection, live);
      expect(live.closed, isFalse);
    },
  );

  test('an offer whose video is in H.264 only is dropped', () async {
    // Ana has the smaller id and offers: as a new connection and as an update.
    final call = Call(group: 'g', me: zoe);
    var made = 0;
    call.connectionsForTests = () async {
      made++;
      return _Connection();
    };
    final connection = _Connection();
    final peer = CallPeer(ana)..connection = connection;
    call.peers[ana] = peer;
    for (final update in [false, true]) {
      await call.handleSignal(ana, {
        't': 'offer',
        'sdp': described(callKinds, video: 'H264'),
        'update': update,
        'n': 1,
      });
    }
    expect(made, 0);
    expect(connection.remote, isEmpty);
    expect(connection.closed, isFalse);
    expect(peer.connection, connection);
  });

  test('offers are taken one a second at most, the newest of them', () async {
    // Ana has the smaller id and offers a new connection as fast as signals go.
    final call = Call(group: 'g', me: zoe)
      ..bridgeForTests = const UiMediaBridge(
        host: '127.0.0.1',
        port: 3478,
        username: 'u',
        password: 'p',
      );
    final made = <DateTime>[];
    call.connectionsForTests = () async {
      made.add(DateTime.now());
      return _Connection();
    };
    call.peers[ana] = CallPeer(ana);
    var n = 0;
    void burst() {
      for (var i = 0; i < 8; i++) {
        unawaited(
          call.onSignal(
            ana,
            jsonEncode({'t': 'offer', 'sdp': described(callKinds), 'n': ++n}),
          ),
        );
      }
    }

    Future<void> taken(int count) =>
        until(() => made.length >= count, what: '$count offers taken');

    // Of the first eight, the last is taken at once.
    burst();
    await taken(1);
    expect(made, hasLength(1));
    // Eight more: only the last, and a second after the one taken.
    burst();
    await taken(2);
    expect(made, hasLength(2));
    expect(
      made[1].difference(made[0]),
      greaterThanOrEqualTo(const Duration(milliseconds: 950)),
    );
    await Future<void>.delayed(const Duration(milliseconds: 1500));
    expect(made, hasLength(2));
  });

  test(
    'a screen shared larger than 8K is scaled from the size its renderer tells',
    () {
      // The renderer draws nothing over 8K; it only tells the size of what it dropped.
      final call = Call(group: 'g', me: zoe);
      expect(call.screenScaleNow, 1);
      call.localScreenTooLarge(10240, 1080);
      // To the 1080 lines of the default quality: 1920 wide.
      expect(call.screenScaleNow, closeTo(10240 / 1920, 0.001));
      call.screenQuality = const ScreenQuality(lines: null);
      // As captured: no side over 8192.
      expect(call.screenScaleNow, closeTo(10240 / 8192, 0.001));
    },
  );

  test('a screen of no known size keeps the scale there is', () async {
    final call = Call(group: 'g', me: zoe);
    // A 4K screen goes out at 1080 lines, the default quality.
    call.localScreen.value = RTCVideoValue.empty.copyWith(
      width: 3840,
      height: 2160,
    );
    await call.rescaleNow();
    expect(call.screenScaleSent, 2);
    // Another screen is being picked: no size until its first picture, while the one
    // before may still send to those who watch.
    call.localScreen.value = RTCVideoValue.empty;
    await call.rescaleNow();
    expect(call.screenScaleSent, 2);
  });

  test('an asking for an offer that had none goes again, a few times, and stops once '
      'one comes', () async {
    renegotiationAfter = const Duration(milliseconds: 20);
    final sent = <(String, Object?)>[];
    signalsForTests = (to, message) => sent.add((to, message['t']));
    addTearDown(() {
      renegotiationAfter = const Duration(seconds: 10);
      signalsForTests = null;
    });
    // This side does not offer to Ana (its id is the larger): it asks her for an offer.
    final call = Call(group: 'g', me: zoe)
      ..bridgeForTests = const UiMediaBridge(
        host: '127.0.0.1',
        port: 3478,
        username: 'u',
        password: 'p',
      );
    call.connectionsForTests = () async => _Connection();
    call.peers[ana] = CallPeer(ana)..connection = _Connection();
    await call.handleSignal(ana, {'t': 'update'});
    expect(sent.first, (ana, 'update'));
    // Lost on its way: asked again, a little later each time.
    await until(() => sent.length > 2, what: 'the update asked for again');
    // An offer comes: no more asking.
    await call.onSignal(
      ana,
      jsonEncode({
        't': 'offer',
        'sdp': described(callKinds),
        'update': true,
        'n': 1,
      }),
    );
    final after = sent.where((s) => s.$2 == 'update').length;
    await Future<void>.delayed(const Duration(milliseconds: 600));
    expect(sent.where((s) => s.$2 == 'update').length, after);
    // Never more than the tries there are.
    expect(after, lessThanOrEqualTo(6));
  });
}
