import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'fake_room.dart';

/// A connection that goes no further than being made, and says when it is closed.
class _Connection implements RTCPeerConnection {
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
  Future<void> close() async => closed = true;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

// Device ids are hex, as long as the logs shorten them to.
const _me = '0000000000000000';
const _ana = 'a000000000000000';
const _bea = 'b000000000000000';

UiPerson _person(String id) =>
    UiPerson(id: id, name: id, short: 'AB', avatar: const UiAvatar(preset: 1));

/// This device in a call in the room of group `g` with Ana and Bea, connected to both.
AppState _inCall() {
  final everyone = [_me, _ana, _bea].map(_person).toList();
  final summary = UiRoomSummary(
    startedMs: 0,
    participants: everyone,
    joined: true,
  );
  final call = Call(group: 'g', me: _me);
  for (final id in [_ana, _bea]) {
    call.peers[id] = CallPeer(id)..connected = true;
  }
  return AppState('/nonexistent/fogata-signals-lost-test')
    ..contacts.groups = [
      UiGroup(
        id: 'g',
        name: 'Friday games',
        localName: '',
        icon: 0,
        members: 3,
        online: 2,
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
    ]
    ..callState.call = call;
}

UiEvent _event(UiEventKind kind, {String? device}) =>
    UiEvent(kind: kind, group: 'g', device: device, received: 0, total: 0);

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;

  setUp(() {
    // The pictures of a participant, made ready as it comes.
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async => call.method == 'createVideoRenderer'
          ? <String, dynamic>{'textureId': ++textures}
          : <String, dynamic>{},
    );
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  test(
    'signals of a participant lost on the way make its connection anew',
    () async {
      final events = StreamController<UiEvent>();
      final state = _inCall()..listenTo(events.stream);
      final call = state.call!
        ..bridgeForTests = const UiMediaBridge(
          host: '127.0.0.1',
          port: 3478,
          username: 'u',
          password: 'p',
        );
      final made = <_Connection>[];
      call.connectionsForTests = () async {
        final connection = _Connection();
        made.add(connection);
        return connection;
      };
      final old = _Connection();
      final lost = call.peers[_ana]!..connection = old;
      final kept = call.peers[_bea]!;
      events.add(_event(UiEventKind.signalsLost, device: _ana));
      await pumpEventQueue();
      // A new participant, offered a new connection (this device has the smaller id),
      // the old one closed; the other participant untouched.
      expect(old.closed, isTrue);
      final again = call.peers[_ana];
      expect(again, isNotNull);
      expect(identical(again, lost), isFalse);
      expect(made, hasLength(1));
      expect(again!.connection, made.single);
      expect(identical(call.peers[_bea], kept), isTrue);
      // Of a call this device is not in, or someone not in it: nothing.
      events
        ..add(_event(UiEventKind.signalsLost, device: 'nobody'))
        ..add(
          const UiEvent(
            kind: UiEventKind.signalsLost,
            group: 'other',
            device: _bea,
            received: 0,
            total: 0,
          ),
        );
      await pumpEventQueue();
      expect(made, hasLength(1));
      expect(identical(call.peers[_bea], kept), isTrue);
      await events.close();
    },
  );

  test('each event is said taken once handled', () async {
    final events = StreamController<UiEvent>();
    var taken = 0;
    final state = fakeRoom(people: 2)
      ..listenTo(events.stream, taken: () => taken++);
    for (var i = 0; i < 5; i++) {
      events.add(_event(UiEventKind.away));
    }
    await pumpEventQueue();
    expect(taken, 5);
    expect(state.stopping.stopped, isFalse);
    await events.close();
  });
}
