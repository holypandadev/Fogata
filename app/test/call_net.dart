// Calls of several devices on one computer, with their signals and connections made up:
// what one call signals reaches the other's [Call.onSignal] while the two have a link, as
// the core carries it, and two connections come up once each holds the other's description
// and the link between their devices is up. Enough of a connection for a call to agree on
// one; nothing is sent on it. Signals arrive in order, each [FakeNet.latency] after it was
// sent, unless the test makes chosen ones late or arrive twice ([FakeNet.delay],
// [FakeNet.duplicate]) or lost ([FakeNet.lose]).

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

  /// Every call [add]ed, those gone from [calls] too: what a test stops once it ended.
  final _everyCall = <Call>[];

  /// The pairs of devices whose link is down.
  final Set<String> _down = {};

  /// Every signal sent between the calls here: from, to, its type, when, and itself.
  final sent =
      <
        ({
          String from,
          String to,
          String type,
          DateTime at,
          Map<String, dynamic> message,
        })
      >[];

  /// Signals that go otherwise than in order after [latency], as the core may carry them:
  /// each signal sent is changed by the first rule that matches it while it has uses left.
  final _rules = <_Rule>[];

  /// The next [times] signals [which] matches take [by] more than [latency]: those sent
  /// after them meanwhile arrive first.
  void delay(SignalMatch which, Duration by, {int times = 1}) =>
      _rules.add(_Rule(which, times, by: by));

  /// The next [times] signals [which] matches arrive twice, the second copy [after] the
  /// first.
  void duplicate(SignalMatch which, Duration after, {int times = 1}) =>
      _rules.add(_Rule(which, times, copyAfter: after));

  /// The next [times] signals [which] matches are lost on their way (counted in [sent]).
  void lose(SignalMatch which, {int times = 1}) =>
      _rules.add(_Rule(which, times, lost: true));

  /// Devices whose calls stand for copies before this one: their signals do not say they
  /// answer a `which` (`w`), and a `which` to them is not taken.
  final Set<String> older = {};

  final Map<int, FakeConnection> _connections = {};
  int _next = 0;

  /// The connections each device made, in order.
  List<FakeConnection> madeBy(String device) =>
      _connections.values.where((c) => c.owner == device).toList();

  void install() => signalsBetweenForTests = _deliver;

  void uninstall() => signalsBetweenForTests = null;

  /// Signals on their way and connections coming up: [stop] drops them.
  final _going = <Timer>{};

  void _later(Duration after, void Function() then) {
    late final Timer timer;
    timer = Timer(after, () {
      _going.remove(timer);
      then();
    });
    _going.add(timer);
  }

  /// Stops every call [add]ed, then drops what is still on its way between them.
  Future<void> stop() async {
    for (final call in _everyCall) {
      await call.stop();
    }
    for (final timer in _going) {
      timer.cancel();
    }
    _going.clear();
  }

  /// [call] takes part, as its device's call, with made-up connections.
  Call add(Call call) {
    calls[call.me] = call;
    _everyCall.add(call);
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
    sent.add((
      from: from,
      to: to,
      type: '${message['t']}',
      at: callClock(),
      message: message,
    ));
    if (!linked(from, to)) return;
    // A copy before this one says nothing of answering a `which`, and takes none.
    if (older.contains(to) && message['t'] == 'which') return;
    if (older.contains(from)) message = {...message}..remove('w');
    final text = jsonEncode(message);
    var wait = latency;
    Duration? copy;
    for (final rule in _rules) {
      if (rule.times > 0 && rule.which(from, to, message)) {
        rule.times--;
        if (rule.lost) return;
        wait += rule.by;
        copy = rule.copyAfter;
        break;
      }
    }
    for (final after in [wait, if (copy != null) wait + copy]) {
      _later(after, () {
        if (linked(from, to)) unawaited(calls[to]?.onSignal(from, text));
      });
    }
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
    _later(connecting, () {
      if (c._up || c.closed || partner.closed || c._failed) return;
      if (partner._failed) return;
      if (!linked(c.owner, partner.owner)) return;
      c._tell(RTCPeerConnectionState.RTCPeerConnectionStateConnected);
      partner._tell(RTCPeerConnectionState.RTCPeerConnectionStateConnected);
    });
  }
}

/// Which signals a rule of [FakeNet] changes.
typedef SignalMatch = bool Function(
  String from,
  String to,
  Map<String, dynamic> message,
);

class _Rule {
  _Rule(
    this.which,
    this.times, {
    this.by = Duration.zero,
    this.copyAfter,
    this.lost = false,
  });

  final SignalMatch which;
  int times;
  final Duration by;
  final Duration? copyAfter;
  final bool lost;
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

  /// Its descriptions as WebRTC's signalling state has them: an answer is taken only
  /// while its own offer waits for one, and refused, as WebRTC refuses it, once both are
  /// set ("stable").
  String _signalling = 'stable';

  /// Fails, as one whose network checks found no way to the other end.
  void fail() => _tell(RTCPeerConnectionState.RTCPeerConnectionStateFailed);

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
        final description = args.first as RTCSessionDescription;
        _signalling = description.type == 'offer'
            ? 'have-local-offer'
            : 'stable';
        local = description.sdp;
        net._later(net.gathering, () {
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
        final description = args.first as RTCSessionDescription;
        if (description.type == 'answer') {
          if (_signalling != 'have-local-offer') {
            return Future<void>.error(
              Exception(
                'Failed to set remote answer sdp: Called in wrong state: '
                '$_signalling',
              ),
            );
          }
          _signalling = 'stable';
        } else {
          _signalling = 'have-remote-offer';
        }
        remote = description.sdp;
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
