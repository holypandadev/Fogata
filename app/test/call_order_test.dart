// Signals between calls go each on its own: one can arrive late, after others sent after
// it, or twice. A state that arrives late does not undo a newer one, and a copy of an
// offer already answered gets the same answer and leaves the connection as it is.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

import 'call_net.dart';
import 'wait.dart';

// The smaller id offers, the larger asks for offers.
const _small = '3000000000000000';
const _large = 'c000000000000000';

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;
  late FakeNet net;
  final made = <Call>[];

  /// Answers WebRTC's calls as a platform with no devices; [more] answers some first.
  void platform([Future<Object?>? Function(MethodCall call)? more]) {
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        final answer = await more?.call(call);
        if (answer != null) return answer;
        return call.method == 'createVideoRenderer'
            ? <String, dynamic>{'textureId': ++textures}
            : <String, dynamic>{};
      },
    );
  }

  setUp(() {
    // The pictures of a participant, made ready as it comes.
    platform();
    net = FakeNet()..install();
  });
  tearDown(() async {
    for (final call in made) {
      await call.stop();
    }
    made.clear();
    net.uninstall();
    signalsForTests = null;
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  Call call(String me) {
    final call = Call(group: 'g', me: me)..bridgeForTests = fakeBridge;
    made.add(call);
    return call;
  }

  /// Two calls connected to each other through [net].
  Future<(Call, Call)> connected() async {
    final small = net.add(call(_small));
    final large = net.add(call(_large));
    await Future.wait([
      small.setParticipants([_small, _large]),
      large.setParticipants([_small, _large]),
    ]);
    await until(
      () => small.peers[_large]!.connected && large.peers[_small]!.connected,
      what: 'connected',
    );
    return (small, large);
  }

  SignalMatch state(String from) =>
      (sender, to, message) => sender == from && message['t'] == 'state';

  /// Waits until every signal sent so far has arrived, the late ones included.
  Future<void> allArrived(Duration late) =>
      Future<void>.delayed(late + net.latency * 4);

  test('a state saying the screen is off that arrives after a newer one saying it is '
      'on neither stops the watching nor the screen being sent', () async {
    const late = Duration(seconds: 2);
    // The state the sharer tells as their connection comes up, before it shares, arrives
    // late at the watcher.
    net.delay(state(_small), late);
    final (sharer, watcher) = await connected();
    // It shares, and tells so; that state arrives first.
    sharer
      ..sharing = true
      ..watch(_large);
    await until(() => watcher.peers[_small]!.sharing, what: 'the sharing told');
    watcher.watch(_small);
    await until(() => sharer.watchers == 1, what: 'the screen watched');
    await allArrived(late);
    expect(watcher.peers[_small]!.sharing, isTrue);
    expect(watcher.watching, _small);
    expect(sharer.watchers, 1);
    expect(sharer.peers[_large]!.watches, _small);
  });

  test('two microphone states that arrive the other way round leave the newer '
      'shown', () async {
    const late = Duration(seconds: 2);
    // The state told as the connection comes up (the microphone on) arrives late.
    net.delay(state(_small), late);
    final (talker, listener) = await connected();
    await talker.setMic(false);
    await until(() => !listener.peers[_small]!.mic, what: 'the mute told');
    await allArrived(late);
    expect(listener.peers[_small]!.mic, isFalse);
  });

  group('states taken by a call alone', () {
    // The numbers are those of a clock in milliseconds, as calls send them.
    Map<String, dynamic> told({required bool mic, int? number, String? call}) =>
        {
          't': 'state',
          's': ?number,
          'sc': ?call,
          'mic': mic,
          'cam': false,
          'screen': false,
          'sound': false,
          'watch': '',
        };

    late Call alone;
    setUp(() async {
      alone = net.add(call(_small));
      await alone.setParticipants([_small, _large]);
    });

    test('states that name no number, from a copy before it, are taken as '
        'they come', () async {
      final peer = alone.peers[_large]!;
      await alone.handleSignal(_large, told(mic: false));
      expect(peer.mic, isFalse);
      // Older or newer, it cannot tell: the last one to come is shown.
      await alone.handleSignal(_large, told(mic: true));
      expect(peer.mic, isTrue);
      await alone.handleSignal(_large, told(mic: false, number: 100000));
      expect(peer.mic, isFalse);
      await alone.handleSignal(_large, told(mic: true));
      expect(peer.mic, isTrue);
    });

    test('a new call of theirs whose numbers start lower is taken', () async {
      final peer = alone.peers[_large]!;
      await alone.handleSignal(_large, {'t': 'again', 'c': 'first'});
      await alone.handleSignal(_large, told(mic: true, number: 100000));
      await alone.handleSignal(_large, told(mic: false, number: 99000));
      expect(peer.mic, isTrue);
      // They left and came back, their clock set a few seconds back meanwhile.
      await alone.handleSignal(_large, {'t': 'again', 'c': 'second'});
      await alone.handleSignal(_large, told(mic: false, number: 90000));
      expect(peer.mic, isFalse);
    });

    test(
      'a late state of their call that ended, after their new call asked, is '
      'dropped and does not raise the number kept',
      () async {
        final peer = alone.peers[_large]!;
        await alone.handleSignal(_large, {'t': 'again', 'c': 'first'});
        await alone.handleSignal(
          _large,
          told(mic: true, number: 100000, call: 'first'),
        );
        // They started again, their clock set back a minute; their new call asks first,
        // and a state of the one that ended arrives after.
        await alone.handleSignal(_large, {'t': 'again', 'c': 'second'});
        await alone.handleSignal(
          _large,
          told(mic: false, number: 100500, call: 'first'),
        );
        expect(peer.mic, isTrue);
        await alone.handleSignal(
          _large,
          told(mic: false, number: 40000, call: 'second'),
        );
        expect(peer.mic, isFalse);
      },
    );

    test('a state of their next call ahead of its asking is dropped; the one after '
        'it is taken', () async {
      final peer = alone.peers[_large]!;
      await alone.handleSignal(_large, {'t': 'again', 'c': 'first'});
      await alone.handleSignal(
        _large,
        told(mic: true, number: 100000, call: 'first'),
      );
      await alone.handleSignal(
        _large,
        told(mic: false, number: 40000, call: 'second'),
      );
      expect(peer.mic, isTrue);
      // Its asking comes, and its state again once their connection is up.
      await alone.handleSignal(_large, {'t': 'again', 'c': 'second'});
      await alone.handleSignal(
        _large,
        told(mic: false, number: 40001, call: 'second'),
      );
      expect(peer.mic, isFalse);
    });

    test(
      'with no call of theirs known yet, a state of another call than the one '
      'before is taken whatever its number',
      () async {
        // They came in again (this side knows none of their calls), their clock set back:
        // a late state of their call before, then one of their new call.
        final peer = alone.peers[_large]!;
        await alone.handleSignal(
          _large,
          told(mic: true, number: 100000, call: 'first'),
        );
        await alone.handleSignal(
          _large,
          told(mic: false, number: 40000, call: 'second'),
        );
        expect(peer.mic, isFalse);
      },
    );

    test('a late answer of their call that ended, to an earlier offer, leaves '
        'their new call known and its signals taken', () async {
      final peer = alone.peers[_large]!;
      final earlier = alone.offerNumberTo(_large)! - 1;
      // Their new call is known and connected.
      await alone.handleSignal(_large, {'t': 'again', 'c': 'second'});
      peer.upForTests();
      final connection = peer.connection;
      // The answer of their call before, to an offer of this side made before, comes late.
      await alone.handleSignal(_large, {
        't': 'answer',
        'sdp': 'v=0',
        'n': earlier,
        'c': 'first',
      });
      expect(peer.theirCall, 'second');
      expect(peer.connection, same(connection));
      expect(peer.connected, isTrue);
      await alone.handleSignal(
        _large,
        told(mic: false, number: 100000, call: 'second'),
      );
      expect(peer.mic, isFalse);
    });

    test('a state that names no call, from a copy before it, is taken whatever '
        'call is known', () async {
      final peer = alone.peers[_large]!;
      await alone.handleSignal(_large, {'t': 'again', 'c': 'first'});
      await alone.handleSignal(_large, told(mic: false));
      expect(peer.mic, isFalse);
      await alone.handleSignal(_large, told(mic: true));
      expect(peer.mic, isTrue);
    });

    test('a state told before their clock jumped forward, arriving late, is '
        'dropped', () async {
      final peer = alone.peers[_large]!;
      // Their clock jumped an hour forward between the two states.
      await alone.handleSignal(_large, told(mic: true, number: 3700000));
      await alone.handleSignal(_large, told(mic: false, number: 100000));
      expect(peer.mic, isTrue);
    });
  });

  for (final atOnce in [false, true]) {
    test('a late offer of the call someone had before leaving and coming back '
        'leaves their new call connected, and its signals taken'
        '${atOnce ? ', when she is back before her leaving is done with' : ''}', () async {
      final (small, large) = await connected();
      final first = small.peers[_large]!.theirCall!;
      // She leaves the room: her call ends, and the room says so.
      net.calls.remove(_large);
      await large.stop();
      final back = net.add(call(_large));
      if (atOnce) {
        // As the room's events have it: her leaving is not waited for, and her coming back
        // follows at once, while her connection here is still being let go of.
        unawaited(small.participantLeft(_large));
      } else {
        await small.setParticipants([_small]);
      }
      // She comes back with a new call.
      await Future.wait([
        small.setParticipants([_small, _large]),
        back.setParticipants([_small, _large]),
      ]);
      await until(
        () =>
            small.peers[_large]?.connected == true &&
            back.peers[_small]?.connected == true,
        what: 'connected again',
      );
      final peer = small.peers[_large]!;
      final second = peer.theirCall!;
      expect(second, isNot(first));
      final connection = peer.connection;
      // An offer of the call that ended, held up on its way.
      await small.handleSignal(_large, {
        't': 'offer',
        'sdp': 'v=0',
        'n': 1,
        'c': first,
      });
      expect(peer.connection, same(connection));
      expect(peer.connected, isTrue);
      expect(peer.theirCall, second);
      // What her new call says is still taken.
      await small.handleSignal(_large, {
        't': 'state',
        'sc': second,
        's': DateTime.now().millisecondsSinceEpoch + 60000,
        'mic': false,
        'cam': false,
        'screen': false,
        'sound': false,
        'watch': '',
      });
      expect(peer.mic, isFalse);
    });
  }

  test(
    'a copy of a first offer already answered gets the same answer and leaves '
    'the connection up',
    () async {
      const after = Duration(milliseconds: 1500);
      net.duplicate(
        (from, to, message) => from == _small && message['t'] == 'offer',
        after,
      );
      final (offerer, answerer) = await connected();
      List<Map<String, dynamic>> answers() => [
        for (final s in net.sent)
          if (s.from == _large && s.type == 'answer') s.message,
      ];
      await until(() => answers().length == 2, what: 'the copy answered');
      final [first, second] = answers();
      expect(second['sdp'], first['sdp']);
      expect(second['n'], first['n']);
      await allArrived(Duration.zero);
      expect(net.madeBy(_large), hasLength(1));
      expect(net.madeBy(_small), hasLength(1));
      expect(answerer.peers[_small]!.connected, isTrue);
      expect(offerer.peers[_large]!.connected, isTrue);
      expect(net.madeBy(_large).single.closed, isFalse);
    },
  );

  test(
    'a copy of a screen sound offer already answered gets the same answer and '
    'leaves its connection open',
    () async {
      const offer =
          'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n'
          'a=rtpmap:111 opus/48000/2\r\na=mid:0\r\n';
      const answer =
          'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n'
          'a=rtpmap:111 opus/48000/2\r\na=mid:0\r\na=answer\r\n';
      var connections = 0;
      final closed = <String>[];
      platform((call) async {
        final id = (call.arguments as Map?)?['peerConnectionId'];
        switch (call.method) {
          case 'createPeerConnection':
            final id = 'sound${++connections}';
            final events = EventChannel('FlutterWebRTC/peerConnectionEvent$id');
            messenger.setMockStreamHandler(
              events,
              MockStreamHandler.inline(onListen: (_, _) {}),
            );
            addTearDown(() => messenger.setMockStreamHandler(events, null));
            return {'peerConnectionId': id};
          case 'createAnswer' || 'getLocalDescription':
            return {'sdp': answer, 'type': 'answer'};
          case 'peerConnectionClose':
            closed.add('$id');
        }
        return null;
      });
      final sent = <Map<String, dynamic>>[];
      signalsForTests = (to, message) => sent.add(message);
      net.uninstall();
      final watcher = call(_large);
      await watcher.setParticipants([_small, _large]);
      watcher.peers[_small]!.sharing = true;
      watcher.watch(_small);
      for (var i = 0; i < 2; i++) {
        await watcher.handleSignal(_small, {'t': 'sound', 'sdp': offer});
      }
      expect(connections, 1);
      expect(closed, isEmpty);
      expect(watcher.peers[_small]!.soundConnection, isNotNull);
      final answers = sent.where((m) => m['t'] == 'sound-answer').toList();
      expect(answers, hasLength(2));
      expect(answers[1]['sdp'], answers[0]['sdp']);
    },
  );
}
