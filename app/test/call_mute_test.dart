import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/host.dart';

/// A connection closed or replaced meanwhile: whatever is asked of it fails.
class _Gone implements RTCPeerConnection {
  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async =>
      throw StateError('the connection is closed');

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A description with the call's four channels, in the codecs a call keeps.
final _described = [
  'v=0',
  for (final (i, kind) in ['audio', 'video', 'video', 'audio'].indexed) ...[
    'm=$kind 9 UDP/TLS/RTP/SAVPF 96',
    kind == 'audio' ? 'a=rtpmap:96 opus/48000/2' : 'a=rtpmap:96 VP8/90000',
    'a=mid:$i',
  ],
  '',
].join('\r\n');

bool _sends(TransceiverDirection? d) =>
    d == TransceiverDirection.SendRecv || d == TransceiverDirection.SendOnly;

/// A connection whose four channels note what they are given, with its voice channel
/// asked for and agreed in [voice]. Its descriptions go as WebRTC's: what a description
/// asks is agreed only once the answer is set. A channel of sound (the voice, the
/// screen's sound) has a send stream while this side's own last description sends on it
/// (from an offer as it is set, from an answer for what the other side takes; a new one
/// starts with its encoding on), and its stream runs (SetSend) only while what is agreed
/// sends and the connection is or has been [up].
class _Connection implements RTCPeerConnection {
  _Connection({TransceiverDirection voice = TransceiverDirection.SendRecv}) {
    this.voice
      ..direction = voice
      ..current = voice;
    // Made by this side's offer: a channel that never sent never had a stream.
    if (!_sends(voice)) voiceSender.fresh();
    // Nothing shared: the screen's sound only receives.
    sound
      ..direction = TransceiverDirection.RecvOnly
      ..current = TransceiverDirection.RecvOnly;
    soundSender.fresh();
    for (final channel in [0, 3]) {
      channels[channel]._sender.onStart = () {
        if (channels[channel].sendOn) _prepared.remove(channel);
      };
    }
    _setSend();
  }

  late final channels = [for (var i = 0; i < 4; i++) _Transceiver(i)];

  _Transceiver get voice => channels[0];
  _Sender get voiceSender => voice._sender;
  _Transceiver get sound => channels[3];
  _Sender get soundSender => sound._sender;

  /// Whether the other side takes this side's voice: when not, what is agreed of it does
  /// not send.
  bool theyTake = true;
  bool closed = false;

  /// The connection is up, or has been ([comeUp]): WebRTC runs what is agreed to send.
  bool up = true;

  /// The directions of sound asked by this side's offer still waiting for its answer.
  Map<int, TransceiverDirection?>? pending;
  String _localType = 'offer';
  int offers = 0;

  /// The channels of sound WebRTC prepared the microphone for: one that came to send
  /// (SetSend false to true calls InitRecording) while its stream did not start. macOS's
  /// audio module then opens the microphone, until a stream starts and stops.
  final _prepared = <int>{};
  bool get prepared => _prepared.isNotEmpty;

  /// Runs (SetSend) what is agreed to send on a connection that is up.
  void _setSend() {
    for (final channel in [0, 3]) {
      final t = channels[channel];
      final on = up && _sends(t.current);
      if (on && !t.sendOn) {
        if (t._sender.stream && t._sender.active) {
          _prepared.remove(channel);
        } else {
          _prepared.add(channel);
        }
      }
      t.sendOn = on;
    }
  }

  /// The connection comes up.
  void comeUp() {
    up = true;
    _setSend();
  }

  /// The connection has never been up: nothing of it runs yet.
  void neverUp() {
    up = false;
    for (final channel in [0, 3]) {
      channels[channel].sendOn = false;
    }
  }

  /// This side's own description, asking [asked]: an offer has a send stream for every
  /// channel it sends on.
  void _own(Map<int, TransceiverDirection?> asked, {required bool answer}) {
    for (final channel in [0, 3]) {
      final t = channels[channel];
      final sends = answer
          ? _sends(t.current)
          : _sends(asked[channel] ?? TransceiverDirection.RecvOnly);
      if (!sends) {
        t._sender.stream = false;
      } else if (!t._sender.stream) {
        t._sender.makeStream();
      }
    }
  }

  /// What is agreed once this side's [asked] directions meet the other side's.
  void _agree(Map<int, TransceiverDirection?> asked) {
    for (final channel in [0, 3]) {
      final want = asked[channel] ?? TransceiverDirection.RecvOnly;
      channels[channel].current = channel != 0 || theyTake
          ? want
          : TransceiverDirection.RecvOnly;
    }
  }

  Map<int, TransceiverDirection?> get _wished => {
    0: voice.direction,
    3: sound.direction,
  };

  @override
  Future<List<RTCRtpTransceiver>> getTransceivers() async => channels;

  /// The channels a new offer adds, in order.
  int _added = 0;

  @override
  Future<RTCRtpTransceiver> addTransceiver({
    MediaStreamTrack? track,
    RTCRtpMediaType? kind,
    RTCRtpTransceiverInit? init,
  }) async {
    final channel = channels[_added++];
    channel
      ..direction = init?.direction
      ..current = null
      ..sendOn = false;
    channel._sender.fresh();
    return channel;
  }

  @override
  Future<RTCSessionDescription> createOffer([
    Map<String, dynamic>? constraints,
  ]) async {
    offers++;
    return RTCSessionDescription(_described, 'offer');
  }

  @override
  Future<RTCSessionDescription> createAnswer([
    Map<String, dynamic>? constraints,
  ]) async => RTCSessionDescription(_described, 'answer');

  @override
  Future<void> setLocalDescription(RTCSessionDescription description) async {
    _localType = description.type ?? 'offer';
    onIceGatheringState?.call(
      RTCIceGatheringState.RTCIceGatheringStateComplete,
    );
    if (_localType == 'offer') {
      pending = _wished;
      _own(_wished, answer: false);
    } else {
      _agree(_wished);
      _own(_wished, answer: true);
      _setSend();
    }
  }

  @override
  Future<void> setRemoteDescription(RTCSessionDescription description) async {
    if (description.type == 'answer' && pending != null) {
      _agree(pending!);
      pending = null;
      _setSend();
    }
  }

  @override
  Future<RTCSessionDescription?> getLocalDescription() async =>
      RTCSessionDescription(_described, _localType);

  @override
  Future<void> close() async => closed = true;

  @override
  Future<void> dispose() async {}

  @override
  Function(RTCPeerConnectionState state)? onConnectionState;

  @override
  Function(RTCTrackEvent event)? onTrack;

  @override
  Function(RTCIceGatheringState state)? onIceGatheringState;

  @override
  Function(RTCIceConnectionState state)? onIceConnectionState;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Transceiver implements RTCRtpTransceiver {
  _Transceiver(int index) : mid = '$index';
  final _sender = _Sender();

  /// The direction asked for, and the one last agreed; its send stream runs (SetSend).
  TransceiverDirection? direction;
  TransceiverDirection? current;
  bool sendOn = false;
  int directionsSet = 0;

  @override
  final String mid;

  @override
  RTCRtpSender get sender => _sender;

  @override
  Future<void> setDirection(TransceiverDirection direction) async {
    directionsSet++;
    this.direction = direction;
  }

  // Linux's and Windows' plugin has no rtpTransceiverGetDirection.
  @override
  Future<TransceiverDirection> getDirection() async =>
      throw MissingPluginException('rtpTransceiverGetDirection');

  @override
  Future<TransceiverDirection?> getCurrentDirection() async => current;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// How a sender takes a change of its parameters.
enum _Answer { takes, refuses, fails, ignores, refusesOnce }

class _Sender implements RTCRtpSender {
  int replaced = 0;
  MediaStreamTrack? sent;

  /// It has a send stream, whose one encoding is on or not.
  bool stream = true;
  bool active = true;
  _Answer answer = _Answer.takes;

  /// As WebRTC's sender (pc/rtp_sender.cc): until its first send stream it keeps one
  /// encoding (init_parameters_), which a change sets and the first stream is made with
  /// (ApplyInitParameters_w); after it, with no stream, it keeps none, a change of one is
  /// refused, and a new stream starts with its encoding on.
  bool hadStream = true;
  bool initActive = true;

  /// Its streams start with their encoding off, whatever was kept.
  bool bornOff = false;

  /// Told when its stream's encoding is turned on.
  void Function()? onStart;

  /// Made by an offer: no stream yet, ever.
  void fresh() {
    stream = false;
    hadStream = false;
    initActive = true;
  }

  void makeStream() {
    active = !bornOff && (hadStream || initActive);
    stream = true;
    hadStream = true;
    initActive = true;
  }

  /// What each change asked, and how long a change takes.
  final asked = <bool>[];
  final parameterSets = <RTCRtpParameters>[];
  Duration takes = Duration.zero;

  @override
  Future<void> replaceTrack(MediaStreamTrack? track) async {
    replaced++;
    sent = track;
  }

  @override
  RTCRtpParameters get parameters => RTCRtpParameters(
    transactionId: 't',
    encodings: [
      if (stream)
        RTCRtpEncoding(active: active, maxBitrate: 0, ssrc: 1)
      else if (!hadStream)
        RTCRtpEncoding(active: initActive, maxBitrate: 0),
    ],
  );

  @override
  Future<bool> setParameters(RTCRtpParameters parameters) async {
    parameterSets.add(parameters);
    final on = parameters.encodings!.single.active;
    asked.add(on);
    if (takes > Duration.zero) await Future<void>.delayed(takes);
    if (!stream && hadStream) return false;
    switch (answer) {
      case _Answer.takes:
        if (stream) {
          active = on;
          if (on) onStart?.call();
        } else {
          initActive = on;
        }
        return true;
      case _Answer.refusesOnce:
        answer = _Answer.takes;
        return false;
      case _Answer.refuses:
        return false;
      case _Answer.fails:
        throw StateError('no');
      case _Answer.ignores:
        // As macOS's plugin: yes, whatever WebRTC did.
        return true;
    }
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// The microphone's stream, as opened.
class _Stream implements MediaStream {
  _Stream({this.breaks = false, this.sound = true});

  /// Stopping its track fails.
  final bool breaks;

  /// It has a track of sound (a shared screen's may not).
  final bool sound;
  late final track = _Track(breaks);

  @override
  List<MediaStreamTrack> getTracks() => [if (sound) track];

  @override
  List<MediaStreamTrack> getAudioTracks() => [if (sound) track];

  @override
  List<MediaStreamTrack> getVideoTracks() => [];

  @override
  Future<void> dispose() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Track implements MediaStreamTrack {
  _Track(this.breaks);
  final bool breaks;
  bool stopped = false;

  @override
  bool enabled = true;

  @override
  String? get id => 'mic';

  @override
  Future<void> stop() async {
    if (breaks) throw StateError('no');
    stopped = true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// [connection], made by the test, taken by [call] as one it made: the directions its
/// voice and screen sound channels were asked for are known to it ([Call.askedForTests]),
/// as for every connection the call makes itself.
void _adopt(Call call, _Connection connection) {
  for (final channel in [0, 3]) {
    final direction = connection.channels[channel].direction;
    if (direction != null) call.askedForTests(connection, channel, direction);
  }
}

/// The methods Linux's and Windows' plugin answers, as its source names them.
final _desktopMethods = {
  for (final m
      in RegExp(r'method_name\(\)\s*\.compare\(\s*"(\w+)"\s*\)').allMatches(
        File('third_party/flutter_webrtc/common/cpp/src/flutter_webrtc.cc')
            .readAsStringSync(),
      ))
    m[1]!,
};

const _ana = 'aaaaaaaa';
const _bea = 'bbbbbbbb';

/// "me" sorts after Ana: she offers, and this side asks her with `update`; "0" sorts
/// before her: this side offers.
const _answering = 'me';
const _offering = '0';

/// WebRTC records for [channel] (0, the voice; 3, the screen's sound) of [peer]'s
/// connection: it has a send stream agreed to send, with its encoding on, on a
/// connection that has been up; whatever its track.
bool _records(CallPeer peer, [int channel = 0]) {
  final connection = peer.connection;
  if (connection is! _Connection) return false;
  final t = connection.channels[channel];
  return t.sendOn && t._sender.stream && t._sender.active;
}

/// What must always hold once the call has done what it was asked: the microphone on,
/// every connection up sends it, its track enabled and its encoding on; off, the voice
/// records nothing and no track of it that goes out is enabled. The screen's sound
/// channel records only where [sound] says it sends ([sound] null: nothing shared).
void _holds(Call call, {bool sound = false}) {
  for (final peer in call.peers.values) {
    final connection = peer.connection;
    if (connection is! _Connection || !peer.connected) continue;
    final sender = connection.voiceSender;
    if (call.mic) {
      expect(_sends(connection.voice.direction), isTrue, reason: peer.id);
      expect(sender.sent?.enabled, isTrue, reason: peer.id);
      expect(!sender.stream || sender.active, isTrue, reason: peer.id);
    } else {
      expect(_records(peer), isFalse, reason: peer.id);
      expect(sender.sent?.enabled ?? false, isFalse, reason: peer.id);
      expect(connection.prepared, isFalse, reason: '${peer.id}, prepared');
    }
    if (!sound) {
      expect(_records(peer, 3), isFalse, reason: '${peer.id}, screen sound');
    }
    expect(
      _sends(connection.sound.direction),
      sound,
      reason: '${peer.id}, screen sound',
    );
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final sent = <(String, Map<String, dynamic>)>[];
  setUp(() {
    sent.clear();
    signalsForTests = (to, message) => sent.add((to, message));
    voiceStartAgainAfter = const [
      Duration(milliseconds: 10),
      Duration(milliseconds: 10),
      Duration(milliseconds: 10),
    ];
  });
  // Every call a test makes ends with it: one left going keeps its timers (an offer or
  // an asking for one sent again 10 s later, a connection tried again), whose signals
  // would reach a later test's list on a slow computer.
  final calls = <Call>[];
  Future<void> endCalls() async {
    for (final call in calls) {
      await call.stop();
    }
    calls.clear();
  }

  tearDown(() async {
    await endCalls();
    signalsForTests = null;
    pinnedHost = HostSystem.linux;
  });

  /// Waits until [done], as long as a slow computer may take; fails after [within].
  Future<void> until(
    bool Function() done, {
    Duration within = const Duration(seconds: 10),
  }) async {
    final end = DateTime.now().add(within);
    while (!done()) {
      if (DateTime.now().isAfter(end)) fail('not done within $within');
      await Future<void>.delayed(const Duration(milliseconds: 5));
    }
  }

  /// [call], ended with the test.
  Call made(Call call) {
    calls.add(call);
    return call;
  }

  Iterable<(String, String)> types() => sent.map((s) => (s.$1, '${s.$2['t']}'));

  /// What starts a new agreement: an offer, or asking the other side for one.
  Iterable<(String, String)> agreements() =>
      types().where((s) => s.$2 == 'offer' || s.$2 == 'update');

  /// Every participant's turn done, twice over: what one turn queued runs too.
  Future<void> settle(Call call) async {
    for (var i = 0; i < 3; i++) {
      await pumpEventQueue();
      await call.voiceSettledForTests();
    }
  }

  /// A call of [me] with Ana on [connection], up unless not [up]; the microphone open
  /// when [mic].
  (Call, CallPeer) inCall(
    _Connection connection, {
    required bool mic,
    String me = _answering,
    bool up = true,
  }) {
    final peer = CallPeer(_ana)
      ..connection = connection
      ..describedForTests();
    if (up) {
      peer.upForTests();
    } else {
      connection.neverUp();
    }
    final call = made(Call(group: 'g', me: me, mic: mic))..peers[_ana] = peer;
    _adopt(call, connection);
    if (mic) call.holdForTests(streams: [_Stream()]);
    return (call, peer);
  }

  /// [peer]'s connection comes up: WebRTC runs what is agreed to send, and the call
  /// hears of it.
  void comeUp(Call call, CallPeer peer) {
    (peer.connection! as _Connection).comeUp();
    call.comeUpForTests(peer);
  }

  /// Ana's offer, of a new agreement on the connection there is unless [anew].
  Future<void> offerFromAna(Call call, {bool anew = false}) =>
      call.handleSignal(_ana, {
        't': 'offer',
        'sdp': _described,
        'update': !anew,
        'n': 1,
      });

  /// Ana's answer to this side's last offer.
  Future<void> answerFromAna(Call call) => call.handleSignal(_ana, {
    't': 'answer',
    'sdp': _described,
    'n': call.offerNumberTo(_ana),
  });

  // One connection closed as the microphone is turned off (made again as it is retried):
  // the others still stop receiving the voice, and everyone is told the microphone is off.
  test('a mute reaches every participant when one connection fails', () async {
    final good = _Connection();
    final call = made(Call(group: 'g', me: 'me', mic: true))
      ..peers[_ana] = (CallPeer(_ana)..connection = _Gone())
      ..peers[_bea] = (CallPeer(_bea)..connection = good);
    await call.setMic(false);
    await settle(call);
    expect(good.voice.direction, TransceiverDirection.RecvOnly);
    expect(good.voiceSender.replaced, 1);
    // "me" sorts after both: each is asked for a new offer.
    expect(types(), contains((_bea, 'update')));
    expect(types(), containsAll([(_ana, 'state'), (_bea, 'state')]));
  });

  test('the screen\'s sound turned off reaches every participant when one connection '
      'fails', () async {
    final good = _Connection();
    final call = made(Call(group: 'g', me: 'me', mic: false))
      ..peers[_ana] = (CallPeer(_ana)..connection = _Gone())
      ..peers[_bea] = (CallPeer(_bea)..connection = good);
    await call.setShareSound(false);
    await settle(call);
    expect(good.channels[3]._sender.replaced, greaterThanOrEqualTo(1));
    expect(types(), containsAll([(_ana, 'state'), (_bea, 'state')]));
  });

  // The voice channel keeps its direction and only stops its encoding, which stops
  // WebRTC's recording; the unmute starts it again with no signal to wait for.
  test(
    'a mute and an unmute on a connection that sends voice agree on nothing',
    () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true);

      await call.setMic(false);
      await settle(call);
      expect(agreements(), isEmpty);
      expect(types(), contains((_ana, 'state')));
      expect(connection.voice.directionsSet, 0);
      expect(connection.voiceSender.active, isFalse);
      expect(connection.voiceSender.sent, isNull);
      _holds(call);
      // Only the encoding's state goes back: a bitrate of 0, as read, would be taken.
      final set = connection.voiceSender.parameterSets.single.encodings!.single;
      expect(set.maxBitrate, isNull);
      expect(set.ssrc, isNull);

      sent.clear();
      final again = _Stream();
      call.holdForTests(streams: [again]);
      await call.setMic(true);
      await settle(call);
      expect(agreements(), isEmpty);
      expect(types(), contains((_ana, 'state')));
      expect(connection.voice.directionsSet, 0);
      expect(connection.voiceSender.asked, [false, true]);
      expect(connection.voiceSender.sent, same(again.track));
      _holds(call);
      expect(peer.connection, same(connection));
    },
  );

  // A connection made while muted never prepared the microphone: its first unmute agrees
  // once, as the voice channel only received; every mute and unmute after it agrees on
  // nothing.
  test(
    'after entering muted, only the first unmute agrees on the voice',
    () async {
      final connection = _Connection(voice: TransceiverDirection.RecvOnly);
      final (call, _) = inCall(connection, mic: false);

      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await settle(call);
      expect(connection.voice.direction, TransceiverDirection.SendRecv);
      expect(agreements(), [(_ana, 'update')]);
      // Agreed: Ana offers, and the voice gets its send stream.
      await offerFromAna(call);
      expect(connection.voice.current, TransceiverDirection.SendRecv);
      _holds(call);

      sent.clear();
      await call.setMic(false);
      await settle(call);
      _holds(call);
      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await settle(call);
      _holds(call);
      await call.setMic(false);
      await settle(call);
      _holds(call);
      expect(agreements(), isEmpty);
      expect(connection.voice.direction, TransceiverDirection.SendRecv);
      expect(connection.voiceSender.asked, [false, true, false]);
      expect(types().where((s) => s.$2 == 'state'), hasLength(3));
    },
  );

  test('a voice channel on a connection not up yet only receives while muted', () async {
    final connection = _Connection();
    // WebRTC would prepare the microphone once it comes up.
    final (call, peer) = inCall(connection, mic: true, up: false);
    await call.setMic(false);
    await settle(call);
    expect(connection.voice.direction, TransceiverDirection.RecvOnly);
    expect(agreements(), [(_ana, 'update')]);
    // It comes up before Ana's offer: up, and still agreed to send, it is kept
    // sending with its encoding stopped.
    comeUp(call, peer);
    await offerFromAna(call);
    await settle(call);
    expect(connection.voice.direction, TransceiverDirection.SendRecv);
    expect(connection.voiceSender.active, isFalse);
    _holds(call);
  });

  // An encoding that cannot be stopped (refused, failed, or answered yes and not done,
  // as macOS's plugin answers) leaves the channel to only receive, as before.
  for (final answer in [_Answer.refuses, _Answer.fails, _Answer.ignores]) {
    test(
      'a mute whose encoding stays on (${answer.name}) only receives',
      () async {
        final connection = _Connection();
        connection.voiceSender.answer = answer;
        final (call, _) = inCall(connection, mic: true);
        await call.setMic(false);
        await settle(call);
        expect(connection.voice.direction, TransceiverDirection.RecvOnly);
        expect(agreements(), [(_ana, 'update')]);
        await offerFromAna(call);
        _holds(call);
      },
    );
  }

  // An encoding that cannot be turned on is tried once more, and then the connection is
  // made anew, at every unmute it happens at.
  test('an unmute whose encoding is refused once is tried again', () async {
    final connection = _Connection();
    final (call, peer) = inCall(connection, mic: true);
    await call.setMic(false);
    await settle(call);
    connection.voiceSender.answer = _Answer.refusesOnce;
    call.holdForTests(streams: [_Stream()]);
    await call.setMic(true);
    await until(() => connection.voiceSender.active);
    await settle(call);
    expect(peer.connection, same(connection));
    _holds(call);
  });

  for (final answer in [_Answer.refuses, _Answer.fails, _Answer.ignores]) {
    test(
      'an unmute whose encoding stays off (${answer.name}) connects anew, each time '
      '(offering)',
      () async {
        final made = <_Connection>[];
        final first = _Connection();
        final (call, peer) = inCall(first, mic: true, me: _offering);
        call.connectionsForTests = () async {
          final next = _Connection(voice: TransceiverDirection.RecvOnly);
          made.add(next);
          return next;
        };
        for (final n in [1, 2]) {
          final connection = peer.connection! as _Connection;
          await call.setMic(false);
          await settle(call);
          _holds(call);
          connection.voiceSender.answer = answer;
          call.holdForTests(streams: [_Stream()]);
          await call.setMic(true);
          await until(() => made.length == n);
          await settle(call);
          // Tried, then 3 times more, before connecting anew.
          expect(
            connection.voiceSender.asked.where((on) => on).length,
            greaterThanOrEqualTo(4),
          );
          expect(connection.closed, isTrue);
          expect(made, hasLength(n));
          final anew = made.last;
          expect(peer.connection, same(anew));
          // Offered with the voice sending; once answered and up, it sends.
          expect(anew.voice.direction, TransceiverDirection.SendRecv);
          await answerFromAna(call);
          comeUp(call, peer);
          await settle(call);
          _holds(call);
        }
      },
    );
  }

  test(
    'an unmute whose encoding stays off connects anew (answering)',
    () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true);
      await call.setMic(false);
      await settle(call);
      connection.voiceSender.answer = _Answer.refuses;
      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await until(() => connection.closed);
      await settle(call);
      // Closed, and Ana asked for a new offer (`again`, which she takes once her end of
      // it is no longer up), again and again until one comes.
      expect(connection.closed, isTrue);
      expect(peer.connection, isNull);
      sent.clear();
      await until(() => types().where((s) => s.$2 == 'again').length >= 2);
      // Her offer comes: a new connection, sending; no more asking.
      final anew = _Connection(voice: TransceiverDirection.RecvOnly);
      call.connectionsForTests = () async => anew;
      await offerFromAna(call, anew: true);
      expect(anew.voice.direction, TransceiverDirection.SendRecv);
      sent.clear();
      await Future<void>.delayed(const Duration(milliseconds: 2200));
      expect(types().where((s) => s.$2 == 'again'), isEmpty);
      await call.stop();
    },
  );

  // A call that ends while the voice is being turned on: a connection closed meanwhile is
  // no reason to make one anew, and nothing is made.
  test('nothing is made anew once the call ended', () async {
    final connection = _Connection();
    final (call, _) = inCall(connection, mic: true, me: _offering);
    await call.setMic(false);
    await settle(call);
    var made = 0;
    call.connectionsForTests = () async {
      made++;
      return _Connection();
    };
    connection.voiceSender
      ..answer = _Answer.refuses
      ..takes = const Duration(milliseconds: 30);
    call.holdForTests(streams: [_Stream()]);
    unawaited(call.setMic(true));
    // The voice being turned on.
    await until(() => connection.voiceSender.asked.contains(true));
    await call.stop();
    await Future<void>.delayed(const Duration(milliseconds: 500));
    expect(made, 0);
  });

  test('the desktop plugin\'s methods are read from its source', () {
    expect(
      _desktopMethods,
      containsAll([
        'getUserMedia',
        'getTransceivers',
        'rtpTransceiverSetDirection',
        'rtpTransceiverGetCurrentDirection',
        'rtpSenderSetParameters',
        'rtpSenderReplaceTrack',
        'mediaStreamTrackSetEnable',
        'trackDispose',
      ]),
    );
    expect(_desktopMethods, isNot(contains('rtpTransceiverGetDirection')));
  });

  // Asked while the one before is still being made: the last asked wins, and nothing
  // of the first is left over.
  test('a mute and an unmute asked at once end unmuted', () async {
    final connection = _Connection();
    final (call, _) = inCall(connection, mic: true);
    connection.voiceSender.takes = const Duration(milliseconds: 300);
    final off = call.setMic(false);
    await until(() => connection.voiceSender.asked.isNotEmpty);
    expect(connection.voiceSender.asked, [false]);
    call.holdForTests(streams: [_Stream()]);
    final on = call.setMic(true);
    await Future.wait([off, on]);
    await settle(call);
    expect(call.mic, isTrue);
    _holds(call);
    expect(agreements(), isEmpty);
  });

  test('an unmute and a mute asked at once end muted', () async {
    final connection = _Connection();
    final (call, _) = inCall(connection, mic: true);
    await call.setMic(false);
    await settle(call);
    sent.clear();
    connection.voiceSender.takes = const Duration(milliseconds: 300);
    call.holdForTests(streams: [_Stream()]);
    final on = call.setMic(true);
    await until(() => connection.voiceSender.asked.length >= 2);
    expect(connection.voiceSender.asked.last, isTrue);
    final off = call.setMic(false);
    await Future.wait([on, off]);
    await settle(call);
    expect(call.mic, isFalse);
    _holds(call);
    expect(connection.voice.direction, TransceiverDirection.SendRecv);
    expect(agreements(), isEmpty);
  });

  // Ana (who offers) pauses this device's voice while it is muted: her offer takes no
  // voice. Taking it again later would have WebRTC prepare the microphone, so the
  // channel only receives until the unmute.
  test('a muted voice the other side stops taking only receives', () async {
    final connection = _Connection();
    final (call, _) = inCall(connection, mic: true);
    await call.setMic(false);
    await settle(call);
    sent.clear();

    connection.theyTake = false;
    await offerFromAna(call);
    expect(types().first, (_ana, 'answer'));
    expect(connection.voice.direction, TransceiverDirection.RecvOnly);
    expect(agreements(), [(_ana, 'update')]);
    _holds(call);

    // When she takes the voice again, this side's answer still only receives.
    connection.theyTake = true;
    await offerFromAna(call);
    expect(connection.voice.current, TransceiverDirection.RecvOnly);
    expect(connection.voiceSender.stream, isFalse);
    _holds(call);
  });

  // This side offers. Made muted; unmuted, and muted again before Ana's answer: her
  // answer agrees on a channel that sends, whose new send stream starts with its
  // encoding on; it is stopped as the answer is taken.
  test(
    'an unmute and a mute before the answer record nothing (offering)',
    () async {
      final connection = _Connection(voice: TransceiverDirection.RecvOnly);
      final (call, _) = inCall(connection, mic: false, me: _offering);
      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await settle(call);
      expect(connection.pending?[0], TransceiverDirection.SendRecv);
      await call.setMic(false);
      await settle(call);
      // Asked again once she answers.
      expect(connection.offers, 1);

      await answerFromAna(call);
      await settle(call);
      _holds(call);
      expect(connection.voiceSender.stream, isTrue);
      expect(connection.voiceSender.active, isFalse);
      await answerFromAna(call);
      await settle(call);
      _holds(call);
    },
  );

  // Offering side, its answer path: a muted connection is kept as it is agreed.
  test('a mute and an unmute agree on nothing (offering)', () async {
    final connection = _Connection();
    final (call, _) = inCall(connection, mic: true, me: _offering);
    await call.setMic(false);
    await settle(call);
    expect(agreements(), isEmpty);
    _holds(call);
    call.holdForTests(streams: [_Stream()]);
    await call.setMic(true);
    await settle(call);
    expect(agreements(), isEmpty);
    _holds(call);
  });

  // The connection is made anew while muted (Ana starts over): the new one never sent,
  // and its voice channel only receives; made anew while on, it sends.
  for (final mic in [false, true]) {
    test('a connection made anew (microphone ${mic ? 'on' : 'off'})', () async {
      final first = _Connection();
      final (call, peer) = inCall(first, mic: true);
      if (!mic) await call.setMic(false);
      await settle(call);

      final second = _Connection(voice: TransceiverDirection.RecvOnly);
      call.connectionsForTests = () async => second;
      await offerFromAna(call, anew: true);
      expect(first.closed, isTrue);
      expect(peer.connection, same(second));
      expect(
        second.voice.direction,
        mic ? TransceiverDirection.SendRecv : TransceiverDirection.RecvOnly,
      );
      comeUp(call, peer);
      await settle(call);
      _holds(call);
    });
  }

  // The screen's sound channel is given its direction again only when its own differs:
  // a step that failed before is mended, and a mute that changes nothing for it agrees
  // on nothing.
  test(
    'a mute mends the screen sound\'s direction only when it differs',
    () async {
      for (final sound in [
        TransceiverDirection.RecvOnly,
        TransceiverDirection.SendRecv,
      ]) {
        sent.clear();
        final connection = _Connection()..channels[3].direction = sound;
        final (call, _) = inCall(connection, mic: true);
        await call.setMic(false);
        await settle(call);
        expect(connection.channels[3].direction, TransceiverDirection.RecvOnly);
        expect(
          connection.channels[3].directionsSet,
          sound == TransceiverDirection.RecvOnly ? 0 : 1,
        );
        expect(
          agreements(),
          sound == TransceiverDirection.RecvOnly ? isEmpty : [(_ana, 'update')],
        );
      }
    },
  );

  /// Ana and Bea, up; Ana's turn busy until the returned completer completes, as with a
  /// signal of hers still being handled.
  (Call, _Stream, _Connection, _Connection, Completer<void>) twoInCall() {
    final ana = _Connection(), bea = _Connection();
    final call = made(Call(group: 'g', me: 'me', mic: true));
    final stream = _Stream();
    call.holdForTests(streams: [stream]);
    for (final (id, connection) in [(_ana, ana), (_bea, bea)]) {
      call.peers[id] = CallPeer(id)
        ..connection = connection
        ..upForTests()
        ..describedForTests();
      _adopt(call, connection);
    }
    final busy = Completer<void>();
    unawaited(call.peers[_ana]!.inTurnForTests(() => busy.future));
    return (call, stream, ana, bea, busy);
  }

  /// Longer than any step here takes, far shorter than a turn that never ends.
  const soon = Duration(seconds: 2);

  // The voice stops going out the moment the mute is asked, to everyone: the track that
  // every sender holds is disabled before anything else, whatever each connection's turn
  // is doing, and a participant whose turn is busy holds up nobody else.
  test(
    'a mute silences every connection at once, whatever a busy turn',
    () async {
      final (call, stream, ana, bea, busy) = twoInCall();
      final off = call.setMic(false);
      expect(stream.track.enabled, isFalse);
      await off.timeout(soon);
      await until(() => stream.track.stopped && !bea.voiceSender.active);
      expect(types(), containsAll([(_ana, 'state'), (_bea, 'state')]));
      // Ana's connection waits for her turn.
      expect(ana.voiceSender.asked, isEmpty);

      busy.complete();
      await settle(call);
      _holds(call);
      expect(agreements(), isEmpty);
    },
  );

  // macOS: the microphone's track is never disabled while a sender holds it (WebRTC-SDK
  // would then mute Apple's voice processing input): a mute takes it off every sender
  // at once instead, before any participant's turn, Ana's busy one included.
  test('a mute on macOS takes the track off every sender at once', () async {
    pinnedHost = HostSystem.macos;
    final (call, stream, ana, bea, busy) = twoInCall();
    // Sent, as the call put it on them.
    ana.voiceSender.sent = stream.track;
    bea.voiceSender.sent = stream.track;
    await call.setMic(false).timeout(soon);
    await until(
      () => ana.voiceSender.sent == null && bea.voiceSender.sent == null,
    );
    expect(stream.track.enabled, isTrue);
    // Ana's step waits for her turn.
    expect(ana.voiceSender.asked, isEmpty);
    busy.complete();
    await settle(call);
    _holds(call);

    final again = _Stream();
    call.holdForTests(streams: [again]);
    await call.setMic(true);
    await settle(call);
    expect(again.track.enabled, isTrue);
    _holds(call);
  });

  // macOS: the voice engine lost its recording (a start that failed): every voice encoding
  // goes off, all before any is on again, so that WebRTC stops recording and starts it
  // afresh its own way; each participant's own step is the only one that sets them.
  group('a recording lost is started again', () {
    test('every voice encoding off, then on', () async {
      final (call, _, ana, bea, busy) = twoInCall();
      busy.complete();
      await settle(call);
      final peers = [call.peers[_ana]!, call.peers[_bea]!];
      expect(peers.every(_records), isTrue);
      final senders = [ana.voiceSender, bea.voiceSender];
      final askedBefore = [for (final s in senders) s.asked.length];
      // When the first starts again, whether every other was off already.
      bool? othersOff;
      for (final (i, sender) in senders.indexed) {
        final before = sender.onStart;
        sender.onStart = () {
          before?.call();
          othersOff ??= senders.indexed.every((e) => e.$1 == i || !e.$2.active);
        };
      }
      await call.recoverVoiceForTests().timeout(soon);
      await settle(call);
      for (final (i, sender) in senders.indexed) {
        expect(sender.asked.skip(askedBefore[i]), [false, true], reason: '$i');
      }
      // WebRTC's recording went false, then true.
      expect(othersOff, isTrue);
      expect(peers.every(_records), isTrue);
      expect(call.voiceRecoveries, 1);
    });

    test('a step of another origin meanwhile leaves the voice off', () async {
      final (call, _, ana, bea, busy) = twoInCall();
      busy.complete();
      await settle(call);
      // Bea's encoding takes a while to change: Ana is off before her.
      bea.voiceSender.takes = const Duration(milliseconds: 150);
      final restart = call.recoverVoiceForTests();
      await until(() => !ana.voiceSender.active);
      // Every participant's step for another reason (the screen's sound's choice).
      final steps = ana.voiceSender.replaced;
      final asked = ana.voiceSender.asked.length;
      call.shareSound = !call.shareSound;
      await until(() => ana.voiceSender.replaced > steps);
      await Future<void>.delayed(const Duration(milliseconds: 60));
      // Not turned on at any moment, before the others are off.
      expect(ana.voiceSender.asked.skip(asked), isNot(contains(true)));
      expect(ana.voiceSender.active, isFalse);
      await restart.timeout(soon);
      await settle(call);
      expect(_records(call.peers[_ana]!), isTrue);
      expect(_records(call.peers[_bea]!), isTrue);
    });

    test(
      'a participant who arrives meanwhile is off, then on with the others',
      () async {
        final (call, _, ana, bea, busy) = twoInCall();
        busy.complete();
        await settle(call);
        bea.voiceSender.takes = const Duration(milliseconds: 150);
        final restart = call.recoverVoiceForTests();
        await until(() => !ana.voiceSender.active);
        // Their stream made with its encoding off: their step must not turn it on.
        final cee = _Connection();
        cee.voiceSender.active = false;
        const ceeId = 'cccccccc';
        final peer = CallPeer(ceeId)
          ..connection = cee
          ..describedForTests();
        call.peers[ceeId] = peer;
        _adopt(call, cee);
        // Their connection comes up: their own step runs, and leaves it off while the
        // others are off.
        call.comeUpForTests(peer);
        await until(
          () => cee.voiceSender.replaced > 0,
          within: const Duration(seconds: 1),
        );
        await Future<void>.delayed(const Duration(milliseconds: 20));
        expect(cee.voiceSender.active, isFalse);
        expect(cee.voiceSender.asked, isNot(contains(true)));
        expect(ana.voiceSender.active, isFalse);
        await restart.timeout(soon);
        await settle(call);
        for (final id in [_ana, _bea, ceeId]) {
          expect(_records(call.peers[id]!), isTrue, reason: id);
        }
      },
    );

    test('a turn-on refused once is tried again by the step', () async {
      final (call, _, ana, bea, busy) = twoInCall();
      busy.complete();
      await settle(call);
      final restart = call.recoverVoiceForTests();
      await until(() => !ana.voiceSender.active && !bea.voiceSender.active);
      ana.voiceSender.answer = _Answer.refusesOnce;
      await restart.timeout(soon);
      await until(() => _records(call.peers[_ana]!));
      expect(_records(call.peers[_bea]!), isTrue);
    });

    test(
      'a participant still connecting is off too, and holds nothing up',
      () async {
        final (call, _, ana, bea, busy) = twoInCall();
        busy.complete();
        await settle(call);
        // Cee's connection has not come up for the call: WebRTC may find it up first and
        // run its stream, so its encoding is turned off too, and on again with the others.
        final cee = _Connection()..neverUp();
        const ceeId = 'cccccccc';
        call.peers[ceeId] = CallPeer(ceeId)
          ..connection = cee
          ..describedForTests();
        _adopt(call, cee);
        final before = [
          ana.voiceSender.asked.length,
          bea.voiceSender.asked.length,
        ];
        await call.recoverVoiceForTests().timeout(
          const Duration(milliseconds: 1500),
        );
        await settle(call);
        expect(ana.voiceSender.asked.skip(before[0]), [false, true]);
        expect(bea.voiceSender.asked.skip(before[1]), [false, true]);
        expect(cee.voiceSender.asked, [false, true]);
        expect(_records(call.peers[_ana]!), isTrue);
        expect(_records(call.peers[_bea]!), isTrue);
      },
    );

    test('given up when the microphone is turned off', () async {
      final (call, _, ana, bea, busy) = twoInCall();
      busy.complete();
      await settle(call);
      final restart = call.recoverVoiceForTests();
      await call.setMic(false);
      await restart.timeout(soon);
      await settle(call);
      expect(call.mic, isFalse);
      expect(ana.voiceSender.active, isFalse);
      expect(bea.voiceSender.active, isFalse);
      _holds(call);
    });
  });

  test('an unmute is not held up by one participant\'s busy turn', () async {
    final (call, first, ana, bea, busy) = twoInCall();
    final again = _Stream();
    await call.setMic(false).timeout(soon);
    // The mute's closing of the microphone done before another one is there.
    await until(() => first.track.stopped);
    call.holdForTests(streams: [again]);
    await call.setMic(true).timeout(soon);
    await until(
      () =>
          bea.voiceSender.active &&
          identical(bea.voiceSender.sent, again.track),
    );
    expect(ana.voiceSender.asked, isEmpty);

    busy.complete();
    await settle(call);
    _holds(call);
    expect(ana.voiceSender.sent, same(again.track));
  });

  // A turn that never ends holds up no request, and the others follow each one; the
  // hung participant gets one step, once free, for all of them.
  test('a hung turn holds up no request of the microphone', () async {
    final (call, first, ana, bea, busy) = twoInCall();
    var stream = first;
    for (final on in [false, true, false, true, false]) {
      if (on) {
        // The mute's closing of the microphone done before another one is there.
        await until(() => stream.track.stopped);
        stream = _Stream();
        call.holdForTests(streams: [stream]);
      }
      await call.setMic(on).timeout(soon);
      await until(
        () =>
            bea.voiceSender.active == on &&
            identical(bea.voiceSender.sent, on ? stream.track : null),
      );
    }
    expect(ana.voiceSender.asked, isEmpty);
    busy.complete();
    await settle(call);
    _holds(call);
    expect(ana.voiceSender.asked, [false]);
  });

  // A mute overtaken by an unmute before it closed the microphone leaves it open and
  // its track enabled again.
  test('a mute overtaken at once leaves the microphone on', () async {
    final connection = _Connection();
    final stream = _Stream();
    final (call, _) = inCall(connection, mic: true);
    call.holdForTests(streams: [stream]);
    final off = call.setMic(false);
    expect(stream.track.enabled, isFalse);
    final on = call.setMic(true);
    await Future.wait([off, on]);
    await settle(call);
    expect(stream.track.enabled, isTrue);
    expect(stream.track.stopped, isFalse);
    _holds(call);
  });

  // Whatever the device does, the connections follow the microphone.
  test(
    'a microphone that fails to close still mutes every connection',
    () async {
      final connection = _Connection();
      final (call, _) = inCall(connection, mic: true);
      call.holdForTests(streams: [_Stream(breaks: true)]);
      await call.setMic(false);
      await settle(call);
      expect(types(), contains((_ana, 'state')));
      _holds(call);
    },
  );

  // A connection that has been up and is not now (ICE checking again): a mute stops the
  // voice's encoding at once, with nothing to agree, whatever the connection's state.
  test(
    'a mute on a connection up before and not now stops the voice at once',
    () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true);
      peer.connected = false;
      await call.setMic(false);
      await settle(call);
      expect(connection.voiceSender.active, isFalse);
      expect(connection.voice.direction, TransceiverDirection.SendRecv);
      expect(agreements(), isEmpty);
    },
  );

  test('no direction is set but where it is kept', () {
    final sources = Directory('lib')
        .listSync(recursive: true)
        .whereType<File>()
        .where((f) => f.path.endsWith('.dart'))
        // With the separators of every system written the same way.
        .map((f) => (f.path.replaceAll(r'\', '/'), f.readAsStringSync()));
    final calls = [
      for (final (path, text) in sources)
        for (final _ in '.setDirection('.allMatches(text)) path,
    ];
    // Call._ask alone: it keeps what it asks for.
    expect(calls, ['lib/src/call.dart']);
  });

  /// A call of this device, which offers, with Ana up and watching its shared screen
  /// ([screenSound]: whether the capture brings sound), the microphone on.
  (Call, CallPeer, _Connection) sharing({required bool screenSound}) {
    final connection = _Connection();
    final (call, peer) = inCall(connection, mic: true, me: _offering);
    call.holdForTests(
      streams: [
        _Stream(),
        null,
        _Stream(sound: screenSound),
      ],
    );
    call
      ..sharing = true
      ..shareSound = true;
    peer.watchesForTests = _offering;
    return (call, peer, connection);
  }

  // A screen's sound sent keeps WebRTC recording: while the recording is started again it
  // is off as well, and on again after, its track and direction as they were.
  test('a recording started again with a screen shared with sound', () async {
    pinnedHost = HostSystem.windows;
    final (call, peer, connection) = sharing(screenSound: true);
    await call.setShareSound(true);
    await settle(call);
    await answerFromAna(call);
    await settle(call);
    _holds(call, sound: true);
    expect(connection.soundSender.active, isTrue);
    final sound = connection.soundSender;
    final voice = connection.voiceSender;
    final track = sound.sent;
    final directions = connection.sound.directionsSet;
    final asked = (voice.asked.length, sound.asked.length);
    // When the voice starts again, whether the screen's sound was off then.
    bool? soundOff;
    final before = voice.onStart;
    voice.onStart = () {
      before?.call();
      soundOff ??= !sound.active;
    };
    await call.recoverVoiceForTests().timeout(soon);
    await settle(call);
    expect(voice.asked.skip(asked.$1), [false, true]);
    expect(sound.asked.skip(asked.$2), [false, true]);
    expect(soundOff, isTrue);
    expect(sound.active, isTrue);
    expect(sound.sent, same(track));
    expect(connection.sound.directionsSet, directions);
    expect(_records(peer), isTrue);
    _holds(call, sound: true);
  });

  // Windows has no silent input: the screen's sound goes out only with the microphone
  // on. Shared with sound, then muted before Ana's answer: her answer agrees on a sound
  // channel that sends, whose new send stream starts with its encoding on; it is stopped
  // as the answer is taken, and the channel only receives once agreed again.
  for (final together in [false, true]) {
    test('a share with sound muted before the answer records nothing (Windows'
        '${together ? ', asked at once' : ''})', () async {
      pinnedHost = HostSystem.windows;
      final (call, peer, connection) = sharing(screenSound: true);
      if (together) {
        unawaited(call.setShareSound(true));
      } else {
        await call.setShareSound(true);
        await settle(call);
        expect(connection.pending?[3], TransceiverDirection.SendRecv);
      }
      await call.setMic(false);
      await settle(call);
      expect(connection.sound.direction, TransceiverDirection.RecvOnly);

      await answerFromAna(call);
      await settle(call);
      expect(_records(peer, 3), isFalse);
      _holds(call);
      await answerFromAna(call);
      await settle(call);
      expect(connection.soundSender.stream, isFalse);
      _holds(call);

      // Unmuted again, the screen's sound goes out again.
      call.holdForTests(streams: [_Stream(), null, _Stream(sound: true)]);
      await call.setMic(true);
      await settle(call);
      await answerFromAna(call);
      await settle(call);
      _holds(call, sound: true);
      expect(connection.soundSender.active, isTrue);
    });
  }

  // macOS sends the screen's sound on connections of its own: the call's channel for it
  // only receives, whatever the microphone.
  test(
    'a share muted on macOS leaves the screen sound channel receiving',
    () async {
      pinnedHost = HostSystem.macos;
      final (call, _, connection) = sharing(screenSound: false);
      await call.setShareSound(true);
      await settle(call);
      await call.setMic(false);
      await settle(call);
      if (connection.pending != null) await answerFromAna(call);
      await settle(call);
      _holds(call);
      expect(connection.sound.direction, TransceiverDirection.RecvOnly);
    },
  );

  // Linux without a silent input (none could be made): as Windows.
  test('a share with sound muted before the answer records nothing (Linux, no '
      'silent input)', () async {
    final (call, peer, connection) = sharing(screenSound: true);
    await call.setShareSound(true);
    await settle(call);
    await call.setMic(false);
    await settle(call);
    await answerFromAna(call);
    await settle(call);
    expect(_records(peer, 3), isFalse);
    await answerFromAna(call);
    await settle(call);
    _holds(call);
  });

  // WebRTC keeps the parameters a sender is given with no stream for its first one: one
  // left with its encoding stopped (this device no longer stops it so, but whatever left
  // it) makes the stream an unmute's agreement makes start with it off. Ana's offer
  // comes before this side's own step: the step at the end of the answer turns it on.
  test(
    'a voice stream made off by what was kept is turned on as agreed',
    () async {
      final connection = _Connection(voice: TransceiverDirection.RecvOnly);
      final (call, peer) = inCall(connection, mic: false);
      await call.setMic(false);
      await settle(call);
      // A muted step leaves a channel that never sent as it is.
      expect(connection.voiceSender.initActive, isTrue);
      connection.voiceSender.initActive = false;

      final busy = Completer<void>();
      unawaited(peer.inTurnForTests(() => busy.future));
      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await offerFromAna(call);
      expect(connection.voiceSender.stream, isTrue);
      expect(connection.voiceSender.active, isTrue);
      busy.complete();
      await settle(call);
      _holds(call);
    },
  );

  // The shared screen captured anew: its sound reaches every sender before the old
  // capture goes, a watcher whose turn is busy included.
  test(
    'a screen captured anew keeps its sound going to a busy watcher',
    () async {
      pinnedHost = HostSystem.windows;
      final (call, peer, connection) = sharing(screenSound: true);
      await call.setShareSound(true);
      await settle(call);
      await answerFromAna(call);
      await settle(call);
      final busy = Completer<void>();
      unawaited(peer.inTurnForTests(() => busy.future));
      final anew = _Stream();
      await call.swapCaptureForTests(anew);
      expect(connection.soundSender.sent, same(anew.track));
      busy.complete();
      await settle(call);
      _holds(call, sound: true);
    },
  );

  // A voice encoding refused on every new connection: the connection is made anew a few
  // times in the call, and then left as it is.
  test('a connection is made anew a few times at most for the voice', () async {
    final first = _Connection();
    final (call, peer) = inCall(first, mic: true, me: _offering);
    await call.setMic(false);
    await settle(call);
    var made = 0;
    call.connectionsForTests = () async {
      made++;
      final next = _Connection(voice: TransceiverDirection.RecvOnly);
      next.voiceSender
        ..answer = _Answer.refuses
        ..bornOff = true;
      return next;
    };
    first.voiceSender.answer = _Answer.refuses;
    call.holdForTests(streams: [_Stream()]);
    await call.setMic(true);
    // Each new connection answered and up as it comes, until the last one is left as
    // it is; then a while more, for one too many.
    var quiet = 0;
    for (var i = 0; i < 2000 && quiet < 40; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 5));
      final connection = peer.connection;
      if (connection is _Connection && connection.pending != null) {
        await answerFromAna(call);
        comeUp(call, peer);
      }
      quiet = made == Call.maxVoiceAnew ? quiet + 1 : 0;
    }
    await settle(call);
    expect(made, Call.maxVoiceAnew);
    expect((peer.connection! as _Connection).closed, isFalse);
  });

  // What every participant's step follows is changed only through setters that have the
  // step run again: an assignment anywhere else would leave a connection behind.
  test('what the sound step follows changes only through its setters', () {
    final sources = [
      for (final f in Directory('lib').listSync(recursive: true))
        if (f is File && f.path.endsWith('.dart')) f.readAsStringSync(),
    ].join('\n');
    // Assignments, not the field's own declaration with its first value.
    // Every kind of write: =, ??= and the compound ones, ++ and --, and a field among
    // those a pattern assigns ((a, b) = ...). Not seen: a change inside an object a field
    // holds, such as a track added to the shared screen's stream; the call adds tracks
    // only to the streams that show what others send.
    int assigned(String field) => RegExp(
      '(?<!(bool|MediaStream\\?|String\\?) )\\b$field\\s*'
      '(?:(?:[-+*/%|&^~]|<<|>>>?|\\?\\?)?=(?!=)|\\+\\+|--)'
      '|(?:\\+\\+|--)\\s*$field\\b'
      '|\\([^()]*\\b$field\\b[^()]*\\)\\s*=(?!=)',
    ).allMatches(sources).length;
    expect(
      {
        for (final field in [
          '_micOn',
          '_sharingNow',
          '_shareSoundNow',
          '_micStreamNow',
          '_screenStreamNow',
          '_silentNow',
          '_silenceComingNow',
          '_upOnceNow',
          '_watchesNow',
          '_voicePausedNow',
          '_replacingNow',
        ])
          field: assigned(field),
      },
      {
        // The setter, and the constructor's first value.
        '_micOn': 2,
        '_sharingNow': 1,
        '_shareSoundNow': 1,
        '_micStreamNow': 1,
        '_screenStreamNow': 1,
        '_silentNow': 1,
        '_silenceComingNow': 1,
        // Call._comeUp; reset as the connection closes; CallPeer.upForTests.
        '_upOnceNow': 3,
        // Call._setWatches; CallPeer.watchesForTests.
        '_watchesNow': 2,
        '_voicePausedNow': 1,
        '_replacingNow': 1,
      },
    );
  });

  // Ana (this side offers) stops taking this device's voice while it is muted, and takes
  // it again: the channel only receives meanwhile, so that WebRTC never comes to send it
  // again with nothing started, which would prepare the microphone (macOS opens it).
  test(
    'a muted voice the other side stops taking only receives (offering)',
    () async {
      final connection = _Connection();
      final (call, _) = inCall(connection, mic: true, me: _offering);
      await call.setMic(false);
      await settle(call);
      expect(agreements(), isEmpty);
      // Her pause: she asks for an offer, and her answer takes no voice.
      await call.handleSignal(_ana, {'t': 'update'});
      connection.theyTake = false;
      await answerFromAna(call);
      await settle(call);
      expect(connection.voice.direction, TransceiverDirection.RecvOnly);
      // Her answer to that, taking the voice again.
      connection.theyTake = true;
      await answerFromAna(call);
      await settle(call);
      expect(connection.prepared, isFalse);
      _holds(call);
    },
  );

  // The screen's sound's encoding not taken after an agreement is tried again, as the
  // voice's is.
  test('the screen sound\'s encoding not taken is tried again', () async {
    pinnedHost = HostSystem.windows;
    final (call, _, connection) = sharing(screenSound: true);
    connection.soundSender
      ..bornOff = true
      ..answer = _Answer.refusesOnce;
    await call.setShareSound(true);
    await settle(call);
    await answerFromAna(call);
    await until(() => connection.soundSender.active);
    await settle(call);
    _holds(call, sound: true);
  });

  // Late voice dropped ([Call._pauseVoiceIn]): Ana's voice is not taken for a moment,
  // and that is in an agreed description before it is taken again, on both sides.
  test(
    'a pause of their voice is agreed before it is lifted (offering)',
    () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true, me: _offering);
      final pause = call.pauseVoiceInForTests(peer);
      await settle(call);
      expect(connection.pending?[0], TransceiverDirection.SendOnly);
      await answerFromAna(call);
      await settle(call);
      expect(connection.voice.current, TransceiverDirection.SendOnly);
      // Lifted once agreed: offered again, taking her voice.
      expect(connection.pending?[0], TransceiverDirection.SendRecv);
      await answerFromAna(call);
      await pause;
      expect(connection.voice.current, TransceiverDirection.SendRecv);
    },
  );

  test(
    'a pause of their voice is agreed before it is lifted (answering)',
    () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true);
      final pause = call.pauseVoiceInForTests(peer);
      await settle(call);
      expect(agreements(), [(_ana, 'update')]);
      // Her offer, answered without taking her voice.
      await offerFromAna(call);
      expect(connection.voice.current, TransceiverDirection.SendOnly);
      await settle(call);
      // Lifted once agreed: asked again.
      expect(agreements(), [(_ana, 'update'), (_ana, 'update')]);
      await offerFromAna(call);
      await pause;
      expect(connection.voice.current, TransceiverDirection.SendRecv);
    },
  );

  // Answering side, an offer of Ana's already waiting in her turn when the pause is
  // asked: answered as it was, before the pause; the pause is still agreed in the
  // answer to her next offer before it is lifted. And one pause at a time.
  test('a pause asked behind an offer of theirs is still agreed', () async {
    final connection = _Connection();
    final (call, peer) = inCall(connection, mic: true);
    final busy = Completer<void>();
    unawaited(peer.inTurnForTests(() => busy.future));
    unawaited(peer.inTurnForTests(() => offerFromAna(call)));
    final pause = call.pauseVoiceInForTests(peer);
    // A second one meanwhile changes nothing.
    final again = call.pauseVoiceInForTests(peer);
    busy.complete();
    await again;
    await settle(call);
    // Her waiting offer answered taking her voice; then the pause asked for.
    expect(types().where((t) => t.$2 == 'answer'), hasLength(1));
    expect(agreements(), [(_ana, 'update')]);
    // Not lifted until agreed.
    await offerFromAna(call);
    expect(connection.voice.current, TransceiverDirection.SendOnly);
    await settle(call);
    expect(agreements(), [(_ana, 'update'), (_ana, 'update')]);
    await offerFromAna(call);
    await pause;
    expect(connection.voice.current, TransceiverDirection.SendRecv);
  });

  // While the silent input is being chosen for a mute, the screen's sound goes on: the
  // share does not say it waits for the microphone.
  test(
    'the screen sound does not wait for the microphone while the silent input '
    'comes',
    () async {
      final (call, _, _) = sharing(screenSound: true);
      call.mic = false;
      expect(call.soundWaitsForMic, isTrue);
      call.mic = true;
      unawaited(call.setMic(false));
      // As asked: the silent input is about to be chosen (Linux).
      expect(call.sharingSound, isTrue);
      expect(call.soundWaitsForMic, isFalse);
      await settle(call);
    },
  );

  // Another screen picked while sharing with sound: the share goes on, and nothing is
  // agreed again for it; the new capture's sound reaches the watcher at once.
  group('another capture in place of the one shared', () {
    const channel = MethodChannel('FlutterWebRTC.Method');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    var captures = 0;

    /// The system's dialog for the next capture, open until answered: with a capture
    /// with sound or without, or closed.
    Completer<String>? dialog;

    /// Linux's silent input, made when asked.
    var silence = false;
    setUp(() {
      captures = 0;
      dialog = null;
      silence = false;
      messenger.setMockStreamHandler(
        const EventChannel('FlutterWebRTC.Event'),
        MockStreamHandler.inline(onListen: (_, _) {}),
      );
      final textures = <int, MockStreamHandlerEventSink>{};
      messenger.setMockMethodCallHandler(channel, (call) async {
        if (!_desktopMethods.contains(call.method)) {
          throw MissingPluginException(call.method);
        }
        switch (call.method) {
          case 'createVideoRenderer':
            final id = textures.length + 1;
            messenger.setMockStreamHandler(
              EventChannel('FlutterWebRTC/Texture$id'),
              MockStreamHandler.inline(
                onListen: (_, sink) => textures[id] = sink,
              ),
            );
            return <String, dynamic>{'textureId': id};
          case 'fogataSilentInput':
            return silence;
          case 'getSources':
            return {
              'sources': [
                {
                  'deviceId': 'mic',
                  'groupId': '',
                  'kind': 'audioinput',
                  'label': 'Desk microphone',
                },
                if (silence)
                  {
                    'deviceId': 'silence',
                    'groupId': '',
                    'kind': 'audioinput',
                    'label': '$silenceLabel ($pid)',
                  },
              ],
            };
          case 'selectAudioInput':
            return null;
          case 'getDisplayMedia':
            captures++;
            final answer = await (dialog?.future ?? Future.value('sound'));
            if (answer == 'cancel') {
              throw PlatformException(
                code: 'getDisplayMedia',
                message: 'cancelled',
              );
            }
            return {
              'streamId': 'screen$captures',
              'audioTracks': [
                if (answer == 'sound')
                  {
                    'id': 'sound$captures',
                    'label': 'sound',
                    'kind': 'audio',
                    'enabled': true,
                  },
              ],
              'videoTracks': [
                {
                  'id': 'picture$captures',
                  'label': 'picture',
                  'kind': 'video',
                  'enabled': true,
                },
              ],
            };
          case 'videoRendererSetSrcObject':
            final id = (call.arguments as Map)['textureId'] as int;
            final stream = (call.arguments as Map)['streamId'] as String;
            if (stream != '') {
              textures[id]?.success({
                'event': 'didTextureChangeVideoSize',
                'id': id,
                'width': 1280,
                'height': 720,
              });
            }
            return null;
        }
        return <String, dynamic>{};
      });
    });
    // The calls end while the plugin still answers.
    tearDown(() async {
      await endCalls();
      messenger.setMockMethodCallHandler(channel, null);
    });

    /// This device, which offers, sharing with sound with Ana watching, agreed; then
    /// another capture asked, its dialog open.
    Future<(Call, CallPeer, _Connection, Future<void>)> replacing() async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true, me: _offering);
      peer.watchesForTests = _offering;
      await call.localScreen.initialize();
      await call.localCamera.initialize();
      await call.shareScreen(null);
      await settle(call);
      expect(connection.sound.direction, TransceiverDirection.SendRecv);
      await answerFromAna(call);
      await settle(call);
      expect(connection.soundSender.sent?.id, 'sound1');
      expect(_records(peer, 3), isTrue);
      dialog = Completer();
      final share = call.shareScreen(null);
      while (captures < 2) {
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
      await settle(call);
      sent.clear();
      return (call, peer, connection, share);
    }

    for (final system in [HostSystem.windows, HostSystem.linux]) {
      test('agrees nothing again, and records nothing muted meanwhile '
          '(${system.name})', () async {
        pinnedHost = system;
        final (call, peer, connection, share) = await replacing();
        final asked = connection.sound.directionsSet;
        // Muted with the dialog open: nothing records, nothing is agreed.
        await call.setMic(false);
        await settle(call);
        expect(_records(peer), isFalse);
        expect(_records(peer, 3), isFalse);
        // On again: the voice goes out at once, and so the screen's sound could.
        call.holdForTests(streams: [_Stream()]);
        await call.setMic(true);
        await settle(call);
        expect(_records(peer), isTrue);
        expect(connection.voiceSender.sent?.enabled, isTrue);
        expect(_records(peer, 3), isTrue);
        expect(agreements(), isEmpty);
        // The new capture: its sound goes out, nothing asked again.
        dialog!.complete('sound');
        await share;
        await settle(call);
        expect(agreements(), isEmpty);
        expect(connection.sound.directionsSet, asked);
        expect(connection.soundSender.sent?.id, 'sound2');
        _holds(call, sound: true);
      });
    }

    // Linux, the microphone off and the screen's sound going through the silent input:
    // it goes on through the dialog; turned on and off meanwhile, the silent input is
    // not wanted (nothing is shared for that moment), and nothing records.
    test('the silent input through a change of capture (linux)', () async {
      silence = true;
      final (call, peer, connection, share) = await replacing();
      await call.setMic(false);
      await settle(call);
      dialog!.complete('sound');
      await share;
      await settle(call);
      expect(_records(peer, 3), isTrue);
      expect(_records(peer), isFalse);

      dialog = Completer();
      final again = call.shareScreen(null);
      while (captures < 3) {
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
      await settle(call);
      sent.clear();
      expect(_records(peer, 3), isTrue);
      call.holdForTests(streams: [_Stream()]);
      await call.setMic(true);
      await settle(call);
      await call.setMic(false);
      await settle(call);
      expect(_records(peer), isFalse);
      expect(_records(peer, 3), isFalse);
      expect(agreements(), isEmpty);
      dialog!.complete('sound');
      await again;
      await settle(call);
      expect(agreements(), isEmpty);
      _holds(call, sound: true);
    });

    // The dialog closed: the share has stopped, for the others too, and the screen's
    // sound channel no longer sends.
    test('a change of capture cancelled stops the share', () async {
      pinnedHost = HostSystem.windows;
      final (call, peer, connection, share) = await replacing();
      await call.setMic(false);
      await settle(call);
      dialog!.complete('cancel');
      await share;
      await settle(call);
      expect(call.sharing, isFalse);
      expect(
        sent.where((s) => s.$2['t'] == 'state').last.$2['screen'],
        isFalse,
      );
      expect(connection.sound.direction, TransceiverDirection.RecvOnly);
      await answerFromAna(call);
      await settle(call);
      expect(_records(peer, 3), isFalse);
      _holds(call);
    });

    // A capture without sound in place of one with: the sound channel only receives
    // once the new capture is in place.
    test('a capture without sound in place of one with', () async {
      pinnedHost = HostSystem.windows;
      final (call, peer, connection, share) = await replacing();
      expect(connection.sound.direction, TransceiverDirection.SendRecv);
      dialog!.complete('none');
      await share;
      await settle(call);
      expect(connection.sound.direction, TransceiverDirection.RecvOnly);
      expect(agreements(), [(_ana, 'offer')]);
      await answerFromAna(call);
      await settle(call);
      expect(_records(peer, 3), isFalse);
      _holds(call);
    });
  });

  group('the microphone opened through the plugin', () {
    const channel = MethodChannel('FlutterWebRTC.Method');
    var opened = 0;
    var refuse = false;
    var silence = false;

    /// How long the system takes to give the microphone (asking first, say).
    var opening = Duration.zero;
    setUp(() {
      opened = 0;
      refuse = false;
      silence = false;
      opening = Duration.zero;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            // As Linux's and Windows' plugin: a method it does not have is missing.
            if (!_desktopMethods.contains(call.method)) {
              throw MissingPluginException(call.method);
            }
            // Linux's silent input, made and listed when asked.
            if (call.method == 'fogataSilentInput') return silence;
            if (call.method == 'getSources') {
              return {
                'sources': [
                  if (silence)
                    {
                      'deviceId': 'silence',
                      'groupId': '',
                      'kind': 'audioinput',
                      'label': '$silenceLabel ($pid)',
                    },
                ],
              };
            }
            if (call.method != 'getUserMedia') return null;
            opened++;
            await Future<void>.delayed(opening);
            if (refuse) throw PlatformException(code: 'no');
            return {
              'streamId': 's$opened',
              'audioTracks': [
                {
                  'id': 'mic$opened',
                  'label': 'mic',
                  'kind': 'audio',
                  'enabled': true,
                },
              ],
              'videoTracks': <Object>[],
            };
          });
    });
    tearDown(() async {
      micOpensAfter = Duration.zero;
      await endCalls();
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    // Linux with its silent input: muted, the screen's sound keeps going out, recorded
    // from the silent input, and the voice does not.
    test('a share with sound muted keeps its sound going (Linux)', () async {
      silence = true;
      final (call, peer, connection) = sharing(screenSound: true);
      await call.setShareSound(true);
      await settle(call);
      await answerFromAna(call);
      await settle(call);
      sent.clear();
      await call.setMic(false);
      await settle(call);
      // Neither stopped nor agreed again for the moment the silent input is chosen.
      expect(connection.soundSender.asked, isNot(contains(false)));
      expect(agreements(), isEmpty);
      _holds(call, sound: true);
      expect(_records(peer, 3), isTrue);
      expect(connection.soundSender.sent, isNotNull);
    });

    // A mute while an unmute still opens the microphone (the system asking first, say):
    // it goes out at once, and the open, once done, lets go of what it opened. Nothing
    // records meanwhile: a voice encoding goes on only with a track to send.
    test('a mute does not wait for a microphone still opening', () async {
      final connection = _Connection();
      final (call, peer) = inCall(connection, mic: true);
      await call.setMic(false);
      await settle(call);
      // Far longer than the mute below may take to answer.
      opening = const Duration(seconds: 3);
      final on = call.setMic(true);
      await until(() => opened == 1);
      // Something else brings the voice channel to the microphone meanwhile.
      await offerFromAna(call);
      expect(_records(peer), isFalse);

      sent.clear();
      await call.setMic(false).timeout(const Duration(seconds: 1));
      final state = sent.where((s) => s.$2['t'] == 'state').last.$2;
      expect(state['mic'], isFalse);
      expect(_records(peer), isFalse);
      await on;
      await settle(call);
      expect(opened, 1);
      _holds(call);
    });

    // The microphone opens late as the call starts (the system asking first): the
    // connection is up before, and its voice encoding was left off meanwhile. The open
    // itself has the step run again, which turns it on: heard, on both sides.
    for (final me in [_offering, _answering]) {
      test(
        'a microphone opened late is heard (${me == _offering ? 'offering' : 'answering'})',
        () async {
          opening = const Duration(milliseconds: 100);
          final connection = _Connection();
          final (call, peer) = inCall(connection, mic: true, me: me);
          call.holdForTests();
          connection.voiceSender.active = false;
          final open = call.openMicForTests();
          await Future<void>.delayed(const Duration(milliseconds: 20));
          expect(connection.voiceSender.active, isFalse);
          await open;
          await settle(call);
          expect(opened, 1);
          expect(connection.voiceSender.active, isTrue);
          expect(connection.voiceSender.sent?.id, 'mic1');
          expect(_records(peer), isTrue);
          _holds(call);
        },
      );
    }

    test('an unmute opens it and every connection sends it', () async {
      final connection = _Connection();
      final (call, _) = inCall(connection, mic: true);
      await call.setMic(false);
      await settle(call);
      await call.setMic(true);
      await settle(call);
      expect(opened, 1);
      expect(connection.voiceSender.sent?.id, 'mic1');
      _holds(call);
    });

    test('an unmute whose microphone cannot be opened leaves it off', () async {
      final connection = _Connection();
      final (call, _) = inCall(connection, mic: true);
      await call.setMic(false);
      await settle(call);
      refuse = true;
      await call.setMic(true);
      await settle(call);
      expect(call.mic, isFalse);
      _holds(call);
    });

    // Hung up while the unmute waits for the microphone (the system asking first, say):
    // nothing is opened after the call ended, and no connection is touched.
    test('an unmute queued when the call ends opens nothing', () async {
      final connection = _Connection();
      final (call, _) = inCall(connection, mic: true);
      await call.setMic(false);
      await settle(call);
      final asked = connection.voiceSender.asked.length;
      micOpensAfter = const Duration(milliseconds: 50);
      final on = call.setMic(true);
      await call.stop();
      await on;
      await Future<void>.delayed(const Duration(milliseconds: 100));
      expect(opened, 0);
      expect(connection.voiceSender.asked, hasLength(asked));
    });
  });
}
