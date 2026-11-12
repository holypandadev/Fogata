// Coming back into a room: the call's first signals from those already in it, sent as soon
// as they hear this device came in, arrive while its call is still being made.

import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'call_net.dart';
import 'fake_room.dart';
import 'wait.dart';

/// A connection that keeps the descriptions set on it, and goes no further.
class _Connection implements RTCPeerConnection {
  final remote = <String>[];

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
  Future<void> close() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

// Device ids are hex, as long as the logs shorten them to. Ana's is the smaller: she
// offers, and this device ([_zoe]) answers and asks her for offers.
const _ana = 'a000000000000000';
const _zoe = 'f000000000000000';

/// An offer of a call's four channels, in the codecs a call keeps.
String _offer(int n) => jsonEncode({
  't': 'offer',
  'sdp': [
    'v=0',
    for (final (i, kind) in ['audio', 'video', 'video', 'audio'].indexed) ...[
      'm=$kind 9 UDP/TLS/RTP/SAVPF 96',
      kind == 'audio' ? 'a=rtpmap:96 opus/48000/2' : 'a=rtpmap:96 VP8/90000',
      'a=mid:$i',
    ],
    '',
  ].join('\r\n'),
  'n': n,
});

const _bridge = UiMediaBridge(
  host: '127.0.0.1',
  port: 3478,
  username: 'u',
  password: 'p',
);

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;
  final sent = <(String, String)>[];

  setUp(() {
    // The pictures of a participant, made ready as it comes.
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async => call.method == 'createVideoRenderer'
          ? <String, dynamic>{'textureId': ++textures}
          : <String, dynamic>{},
    );
    sent.clear();
    signalsForTests = (to, message) => sent.add((to, '${message['t']}'));
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
    signalsForTests = null;
  });

  test(
    'offers that come while the call starts are answered once it can',
    () async {
      final call = Call(group: 'g', me: _zoe);
      // Before the call has its media bridge, and before the room says who is in it.
      await call.onSignal(_ana, _offer(1));
      // The room says Ana is in it while the call still starts: no connection is made yet.
      await call.setParticipants([_zoe, _ana]);
      await call.onSignal(_ana, _offer(2));
      expect(call.peers, isEmpty);
      expect(call.signalsWaiting, 2);
      // Started: the connection with Ana is made, and her offers are answered.
      final made = <_Connection>[];
      call
        ..bridgeForTests = _bridge
        ..connectionsForTests = () async {
          final connection = _Connection();
          made.add(connection);
          return connection;
        };
      await call.setParticipants([_zoe, _ana]);
      await until(
        () => made.length == 2 && made.every((c) => c.remote.length == 1),
        what: 'both offers taken',
      );
      expect(call.signalsWaiting, 0);
      // And she is asked for an offer at once, in case she kept a connection from before.
      expect(sent.first, (_ana, 'again'));
    },
  );

  test('signals from someone not in the room still get nothing', () async {
    final call = Call(group: 'g', me: _zoe)..bridgeForTests = _bridge;
    final made = <_Connection>[];
    call.connectionsForTests = () async {
      final connection = _Connection();
      made.add(connection);
      return connection;
    };
    await call.onSignal(_ana, _offer(1));
    // The room says who is in it, without her: what she sent goes.
    await call.setParticipants([_zoe, 'b000000000000000']);
    expect(call.signalsWaiting, 0);
    expect(call.peers.keys.toList(), ['b000000000000000']);
    expect(made, isEmpty);
  });

  test(
    "signals that come before the room's call is made are handed to it",
    () async {
      final events = StreamController<UiEvent>();
      final state = fakeRoom(people: 1)
        ..callState.call = null
        ..listenTo(events.stream);
      state.soundServer = () async => true;
      // Ana is in the room with this device.
      final summary = UiRoomSummary(
        startedMs: 0,
        participants: [
          me,
          const UiPerson(
            id: _ana,
            name: 'Ana',
            short: 'AB',
            avatar: UiAvatar(preset: 1),
          ),
        ],
        joined: true,
      );
      state.contacts.groups = [
        UiGroup(
          id: 'g',
          name: 'Friday games',
          localName: '',
          icon: 0,
          members: 2,
          online: 1,
          manager: true,
          rank: 0,
          outranks: const [],
          maxMembers: 20,
          maxInRoom: 8,
          voiceRooms: [
            UiVoiceRoom(id: AppState.firstRoom, name: '', room: summary),
          ],
          room: summary,
          openInvites: 0,
        ),
      ];
      events.add(
        UiEvent(
          kind: UiEventKind.signal,
          group: 'g',
          device: _ana,
          text: _offer(1),
          received: 0,
          total: 0,
        ),
      );
      await pumpEventQueue();
      expect(state.signalsKeptFor('g'), 1);
      await state.startCallForTests('g');
      expect(state.signalsKeptFor('g'), 0);
      expect(state.call?.signalsWaiting, 1);
      await state.endCallForTests();
      await events.close();
    },
  );

  group('leaving the room and entering it again', () {
    // As the lab's check (call_test.dart's rejoin step): the call is back within 6 s, the
    // normal 1-4 s with a margin.
    const within = Duration(seconds: 6);
    // The device that comes back takes this long to start its call (its media bridge and
    // microphone), about what a Mac takes; the one that stayed hears it is back, and its
    // room's refresh names the other, this long after it entered.
    const starting = Duration(seconds: 2);
    const heard = Duration(milliseconds: 300);
    late FakeNet net;
    setUp(() => net = FakeNet()..install());
    tearDown(() => net.uninstall());

    /// [leaves] leaves the room [stays] stays in and enters it again: how long until the
    /// two are connected again, from the moment it entered.
    Future<Duration> rejoin(String stays, String leaves) async {
      final stayer = net.add(
        Call(group: 'g', me: stays)..bridgeForTests = _bridge,
      );
      final before = net.add(
        Call(group: 'g', me: leaves)..bridgeForTests = _bridge,
      );
      await Future.wait([
        stayer.setParticipants([stays, leaves]),
        before.setParticipants([stays, leaves]),
      ]);
      await until(
        () =>
            stayer.peers[leaves]?.connected == true &&
            before.peers[stays]?.connected == true,
        what: 'connected at first',
      );
      // It leaves: nothing reaches it until it is in the room again.
      net.calls.remove(leaves);
      await stayer.setParticipants([stays]);
      final entered = DateTime.now();
      final back = net.add(Call(group: 'g', me: leaves));
      unawaited(
        Future<void>.delayed(heard, () async {
          await stayer.setParticipants([stays, leaves]);
        }),
      );
      // Its own room's refresh names the other while its call still starts.
      unawaited(
        Future<void>.delayed(heard, () async {
          await back.setParticipants([stays, leaves]);
        }),
      );
      unawaited(
        Future<void>.delayed(starting, () async {
          back.bridgeForTests = _bridge;
          await back.setParticipants([stays, leaves]);
        }),
      );
      await until(
        () =>
            stayer.peers[leaves]?.connected == true &&
            back.peers[stays]?.connected == true,
        what: 'connected again',
      );
      return DateTime.now().difference(entered);
    }

    test('the call is back soon when the one that stays offers', () async {
      // The smaller id offers: the one that stayed offers as it hears the other is back,
      // while the other's call is still starting.
      final took = await rejoin('1000000000000000', 'e000000000000000');
      expect(took, lessThan(within));
    });

    test('the call is back soon when the one that comes back offers', () async {
      final took = await rejoin('e100000000000000', '1100000000000000');
      expect(took, lessThan(within));
    });
  });
}
