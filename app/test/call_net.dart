// Calls of several devices on one computer, with their signals and connections made up:
// what one call signals reaches the other's [Call.onSignal] while the two have a link, as
// the core carries it, and two connections come up once each holds the other's description
// and the link between their devices is up. Enough of a connection for a call to agree on
// one; nothing is sent on it.

import 'dart:async';
import 'dart:convert';

import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';

const fakeBridge = UiMediaBridge(
  host: '127.0.0.1',
  port: 3478,
  username: 'u',
  password: 'p',
);

/// The devices' calls, the links between them and the connections they made.
class FakeNet {
  FakeNet({
    this.latency = const Duration(milliseconds: 50),
    this.gathering = const Duration(milliseconds: 300),
    this.connecting = const Duration(milliseconds: 300),
  });

  /// How long a signal takes, a connection's candidates take to gather, and two
  /// connections that agreed take to come up.
  final Duration latency;
  final Duration gathering;
  final Duration connecting;

  /// The call of each device, by its id; a device without one drops what comes to it (as
  /// the core of a device in no room does).
  final Map<String, Call> calls = {};

  /// The pairs of devices whose link is down.
  final Set<String> _down = {};

  /// Every signal sent between the calls here: from, to, its type, when.
  final sent = <({String from, String to, String type, DateTime at})>[];

  final Map<int, FakeConnection> _connections = {};
  int _next = 0;

  /// The connections each device made, in order.
  List<FakeConnection> madeBy(String device) =>
      _connections.values.where((c) => c.owner == device).toList();

  void install() => signalsBetweenForTests = _deliver;

  void uninstall() => signalsBetweenForTests = null;

  /// [call] takes part, as its device's call, with made-up connections.
  Call add(Call call) {
    calls[call.me] = call;
    call.connectionsForTests = () async => FakeConnection._(this, call.me);
    return call;
  }

  static String _pair(String a, String b) =>
      a.compareTo(b) < 0 ? '$a $b' : '$b $a';

  bool linked(String a, String b) => !_down.contains(_pair(a, b));

  /// The link between [a] and [b] goes down: what either sends the other is lost, and the
  /// connections between them fail.
  void cut(String a, String b) {
    _down.add(_pair(a, b));
    for (final c in _connections.values) {
      final partner = c._partner;
      if (partner != null && {c.owner, partner.owner}.containsAll([a, b])) {
        c._tell(RTCPeerConnectionState.RTCPeerConnectionStateFailed);
      }
    }
  }

  /// The link between [a] and [b] is back.
  void mend(String a, String b) {
    _down.remove(_pair(a, b));
    for (final c in _connections.values.toList()) {
      _check(c);
    }
  }

  void _deliver(String from, String to, Map<String, dynamic> message) {
    if (!calls.containsKey(from)) return;
    sent.add((from: from, to: to, type: '${message['t']}', at: DateTime.now()));
    if (!linked(from, to)) return;
    final text = jsonEncode(message);
    Future<void>.delayed(latency, () {
      if (linked(from, to)) unawaited(calls[to]?.onSignal(from, text));
    });
  }

  /// Comes up with its partner when each holds the other's description.
  void _check(FakeConnection c) {
    final partner = c._partner;
    if (c.closed ||
        c._failed ||
        c.local == null ||
        partner == null ||
        partner.closed ||
        partner._failed ||
        partner.local == null ||
        partner._partner != c ||
        !linked(c.owner, partner.owner) ||
        c._up) {
      return;
    }
    Future<void>.delayed(connecting, () {
      if (c._up || c.closed || partner.closed || c._failed) return;
      if (partner._failed) return;
      if (!linked(c.owner, partner.owner)) return;
      c._tell(RTCPeerConnectionState.RTCPeerConnectionStateConnected);
      partner._tell(RTCPeerConnectionState.RTCPeerConnectionStateConnected);
    });
  }
}

final _fakeId = RegExp(r'a=fake:(\d+)');

/// A connection that keeps the descriptions set on it.
class FakeConnection implements RTCPeerConnection {
  FakeConnection._(this.net, this.owner) : id = net._next++ {
    net._connections[id] = this;
  }

  final FakeNet net;
  final String owner;
  final int id;
  String? local;
  String? remote;
  bool closed = false;
  bool _up = false;

  /// Once failed, it never comes up again, as a real one.
  bool _failed = false;

  /// The connection whose description this one holds.
  FakeConnection? get _partner {
    final match = _fakeId.firstMatch(remote ?? '');
    return match == null ? null : net._connections[int.parse(match[1]!)];
  }

  void _tell(RTCPeerConnectionState state) {
    if (closed) return;
    _up = state == RTCPeerConnectionState.RTCPeerConnectionStateConnected;
    if (state == RTCPeerConnectionState.RTCPeerConnectionStateFailed) {
      _failed = true;
    }
    onConnectionState?.call(state);
  }

  /// A description of a call's four channels, in the codecs a call keeps, that names this
  /// connection.
  String get _description => [
    'v=0',
    'a=fake:$id',
    for (final (i, kind) in ['audio', 'video', 'video', 'audio'].indexed) ...[
      'm=$kind 9 UDP/TLS/RTP/SAVPF 96',
      kind == 'audio' ? 'a=rtpmap:96 opus/48000/2' : 'a=rtpmap:96 VP8/90000',
      'a=mid:$i',
    ],
    '',
  ].join('\r\n');

  @override
  Function(RTCPeerConnectionState state)? onConnectionState;

  @override
  Function(RTCTrackEvent event)? onTrack;

  @override
  Function(RTCIceGatheringState state)? onIceGatheringState;

  @override
  Function(RTCIceConnectionState state)? onIceConnectionState;

  @override
  Future<void> close() async {
    closed = true;
    _up = false;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    final args = invocation.positionalArguments;
    switch (invocation.memberName) {
      case #createOffer:
        return Future.value(RTCSessionDescription(_description, 'offer'));
      case #createAnswer:
        return Future.value(RTCSessionDescription(_description, 'answer'));
      case #setLocalDescription:
        local = (args.first as RTCSessionDescription).sdp;
        Future<void>.delayed(net.gathering, () {
          onIceGatheringState?.call(
            RTCIceGatheringState.RTCIceGatheringStateComplete,
          );
        });
        net._check(this);
        final partner = _partner;
        if (partner != null) net._check(partner);
        return Future<void>.value();
      case #getLocalDescription:
        return Future<RTCSessionDescription?>.value(
          local == null ? null : RTCSessionDescription(local, 'offer'),
        );
      case #setRemoteDescription:
        remote = (args.first as RTCSessionDescription).sdp;
        net._check(this);
        return Future<void>.value();
      case #addTransceiver:
        return Future<RTCRtpTransceiver>.value(_Transceiver());
      case #getTransceivers:
        return Future.value(<RTCRtpTransceiver>[]);
      case #addCandidate || #dispose:
        return Future<void>.value();
      case #getStats:
        return Future.value(<StatsReport>[]);
    }
    return super.noSuchMethod(invocation);
  }
}

class _Transceiver implements RTCRtpTransceiver {
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
