// A signal of an older call of a participant's device, held up on its way, arrives once
// the two devices are connected with their calls of now: it must not replace the call
// there is, whether or not this side remembers that older call as ended.

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

import 'call_net.dart';
import 'fake_time.dart';
import 'wait.dart';

// The smaller id offers, the larger asks for offers.
const _small = '2500000000000000';
const _large = 'd500000000000000';

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;
  late FakeNet net;

  /// [inCallTime] with every call of [net] stopped once the test ended ([FakeNet.stop]).
  Future<void> inNetTime(Future<void> Function() body) =>
      inCallTime(body, stop: net.stop);

  setUp(() {
    // The pictures of a participant, made ready as it comes.
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async {
        if (call.method != 'createVideoRenderer') return <String, dynamic>{};
        final events = EventChannel('FlutterWebRTC/Texture${++textures}');
        messenger.setMockStreamHandler(
          events,
          MockStreamHandler.inline(onListen: (_, _) {}),
        );
        addTearDown(() => messenger.setMockStreamHandler(events, null));
        return <String, dynamic>{'textureId': textures};
      },
    );
    net = FakeNet()..install();
  });
  tearDown(() {
    net.uninstall();
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      null,
    );
  });

  bool up(Call small, Call large) =>
      small.peers[_large]?.connected == true &&
      large.peers[_small]?.connected == true;

  /// Two calls connected to each other.
  Future<(Call, Call)> connected() async {
    final small = net.add(
      Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
    );
    final large = net.add(
      Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
    );
    await Future.wait([
      small.setParticipants([_small, _large]),
      large.setParticipants([_small, _large]),
    ]);
    await until(() => up(small, large), what: 'connected');
    return (small, large);
  }

  // An offer reaches the larger side, an asking the smaller.
  for (final (type, sender) in [('offer', _small), ('again', _large)]) {
    test(
      'a late $type of a call both devices made before starting again leaves '
      'their calls of now connected',
      () => inNetTime(() async {
        // The first of it is copied, and the copy held up past both starting again.
        const held = Duration(seconds: 8);
        net.duplicate(
          (from, to, message) => from == sender && message['t'] == type,
          held,
        );
        final (small1, large1) = await connected();
        // Both start again at once (the computer woke, the network came back): their
        // new calls know nothing of the ones before.
        await small1.stop();
        await large1.stop();
        final (small, large) = await connected();
        final smallPeer = small.peers[_large]!;
        final largePeer = large.peers[_small]!;
        final (smallKnows, largeKnows) = (
          smallPeer.theirCall,
          largePeer.theirCall,
        );
        final connections = (smallPeer.connection, largePeer.connection);
        final copied = net.sent.where(
          (s) => s.from == sender && s.type == type,
        );
        final stale = copied.first.message['c'];
        expect(
          stale,
          isNot(sender == _small ? largePeer.theirCall : smallPeer.theirCall),
        );
        // The copy arrives, and well past every try after it.
        await Future<void>.delayed(const Duration(seconds: 40));
        expect(up(small, large), isTrue);
        expect((smallPeer.connection, largePeer.connection), connections);
        expect(smallPeer.theirCall, smallKnows);
        expect(largePeer.theirCall, largeKnows);
        // The older call is known as ended now: another copy of it asks nothing.
        expect(
          (sender == _small ? largePeer : smallPeer).endedCalls,
          contains(stale),
        );
      }),
    );
  }

  /// The questions [from] sent, in order.
  List<String> questions(String from) => [
    for (final s in net.sent)
      if (s.from == from && s.type == 'which') '${s.message['k']}',
  ];

  /// The id of [call], as its signals name it.
  String callOf(Call call) =>
      '${net.sent.lastWhere((s) => s.from == call.me && s.message['c'] != null).message['c']}';

  /// A late signal of [type] from [from]'s call [stale], which neither side remembers,
  /// arrives at the other device's call [to] now.
  /// Unless [answers], it comes from a copy before this one (no `w`).
  Future<void> late(
    Call to,
    String from,
    String type,
    String stale, {
    bool answers = true,
  }) {
    final first = net.sent.firstWhere((s) => s.type == 'offer').message;
    return to.onSignal(
      from,
      jsonEncode({
        't': type,
        if (type == 'offer') ...{'sdp': first['sdp'], 'n': first['n']},
        'c': stale,
        if (answers) 'w': 1,
      }),
    );
  }

  /// The larger device's call ends and it comes back at once with a new one, which the
  /// room never shows the smaller one as gone ([heardLeaving] false) or does: how long
  /// the two take to be connected again from the moment its new call has started.
  Future<(Call, Duration)> comesBack(
    Call small, {
    required bool heardLeaving,
  }) async {
    await net.calls[_large]!.stop();
    net.calls.remove(_large);
    if (heardLeaving) {
      await small.setParticipants([_small]);
      await small.setParticipants([_small, _large]);
    }
    final back = net.add(
      Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
    );
    final started = callClock();
    await back.setParticipants([_small, _large]);
    await until(() => up(small, back), what: 'connected again');
    return (back, callClock().difference(started));
  }

  test(
    'a new call that is real is taken one question later, when the room never '
    'showed its device gone',
    () => inNetTime(() async {
      final (small, _) = await connected();
      // The room showed it gone: no call of theirs is known, and nothing is asked.
      final (_, heard) = await comesBack(small, heardLeaving: true);
      expect(questions(_small), isEmpty);
      final (back, unheard) = await comesBack(small, heardLeaving: false);
      // Its asking, the question and its answer, the offer and its answer (two more
      // signals than with no question), each description's candidates, and the
      // connection coming up; the room showing it gone spared the offer's time.
      expect(questions(_small), hasLength(1));
      expect(
        unheard,
        lessThanOrEqualTo(
          net.latency * 5 +
              net.gathering * 2 +
              net.connecting +
              const Duration(milliseconds: 20),
        ),
      );
      expect(heard, lessThan(unheard));
      expect(small.peers[_large]!.theirCall, callOf(back));
      expect(small.peers[_large]!.asking, isFalse);
    }),
  );

  test(
    'a lost answer is asked for again, with the same token',
    () => inNetTime(() async {
      final (small, _) = await connected();
      net.lose((from, to, message) => message['t'] == 'this');
      final (_, took) = await comesBack(small, heardLeaving: false);
      final asked = questions(_small);
      expect(asked, hasLength(2));
      expect(asked.toSet(), hasLength(1));
      // The second question goes [Call.whichEvery] after the first.
      expect(took, lessThan(Call.whichEvery + const Duration(seconds: 2)));
    }),
  );

  test(
    'a late signal of an older call while their call now has not been heard from '
    'yet: the answer names the call now, and it is connected to',
    () => inNetTime(() async {
      final (small, _) = await connected();
      final before = small.peers[_large]!.theirCall!;
      // The larger device starts again; its new call's asking is lost on the way.
      net.lose(
        (from, to, message) => from == _large && message['t'] == 'again',
      );
      await net.calls[_large]!.stop();
      final back = net.add(
        Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
      );
      await back.setParticipants([_small, _large]);
      // A late asking of a call before both arrives first.
      await late(small, _large, 'again', '0123456789abcdef');
      await until(() => up(small, back), what: 'connected again');
      final peer = small.peers[_large]!;
      expect(peer.theirCall, callOf(back));
      expect(peer.endedCalls, containsAll([before, '0123456789abcdef']));
    }),
  );

  test(
    'an answer made before a newer call of theirs was held does not end that '
    'call',
    () => inNetTime(() async {
      final (small, large) = await connected();
      // Answers are held up on their way.
      net.delay(
        (from, to, message) => message['t'] == 'this',
        const Duration(seconds: 4),
        times: 100,
      );
      await late(small, _large, 'again', '0123456789abcdef');
      await until(
        () => net.sent.any((s) => s.type == 'this'),
        what: 'the answer sent',
      );
      // Then their device starts again: its new call's asking is held in place of the
      // late one, and the answer on its way names the call that just ended.
      await large.stop();
      final back = net.add(
        Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
      );
      await back.setParticipants([_small, _large]);
      await until(() => up(small, back), what: 'connected to the new call');
      await Future<void>.delayed(const Duration(seconds: 10));
      expect(up(small, back), isTrue);
      final peer = small.peers[_large]!;
      expect(peer.theirCall, callOf(back));
      expect(peer.endedCalls, isNot(contains(callOf(back))));
    }),
  );

  test(
    'an answer that comes after the questions stopped is still taken',
    () => inNetTime(() async {
      final (small, _) = await connected();
      // Each answer takes longer than all the questions together.
      net.delay(
        (from, to, message) => message['t'] == 'this',
        Call.whichEvery * Call.whichTries + const Duration(seconds: 2),
        times: 100,
      );
      final (back, _) = await comesBack(small, heardLeaving: false);
      expect(small.peers[_large]!.theirCall, callOf(back));
    }),
  );

  test(
    'both sides asking at once each answer, and both stay connected',
    () => inNetTime(() async {
      final (small, large) = await connected();
      final connections = (
        small.peers[_large]!.connection,
        large.peers[_small]!.connection,
      );
      // An offer waits its turn (one a second): the asking goes once its question is
      // out, and the two questions cross.
      final offered = late(large, _small, 'offer', 'fedcba9876543210');
      await until(
        () => questions(_large).isNotEmpty,
        what: 'the larger side asking',
      );
      await late(small, _large, 'again', '0123456789abcdef');
      await offered;
      expect(small.peers[_large]!.asking, isTrue);
      expect(large.peers[_small]!.asking, isTrue);
      await Future<void>.delayed(const Duration(seconds: 1));
      expect(questions(_small), hasLength(1));
      expect(questions(_large), hasLength(1));
      expect(small.peers[_large]!.asking, isFalse);
      expect(large.peers[_small]!.asking, isFalse);
      expect(up(small, large), isTrue);
      expect((
        small.peers[_large]!.connection,
        large.peers[_small]!.connection,
      ), connections);
      expect(small.peers[_large]!.endedCalls, contains('0123456789abcdef'));
      expect(large.peers[_small]!.endedCalls, contains('fedcba9876543210'));
    }),
  );

  test(
    'questions and answers that arrive twice, late signals that come again, and '
    'answers to no question change nothing',
    () => inNetTime(() async {
      bool asks(String from, String to, Map<String, dynamic> message) =>
          message['t'] == 'which' || message['t'] == 'this';
      net.duplicate(asks, const Duration(milliseconds: 500), times: 10);
      final (small, large) = await connected();
      final peer = small.peers[_large]!;
      final (known, connection) = (peer.theirCall, peer.connection);
      // The same late asking twice before the answer, and once after it.
      await late(small, _large, 'again', '0123456789abcdef');
      await late(small, _large, 'again', '0123456789abcdef');
      await Future<void>.delayed(const Duration(seconds: 2));
      await late(small, _large, 'again', '0123456789abcdef');
      // An answer naming another call with a token never asked, or with none.
      for (final token in ['00000000000000ff', null]) {
        await small.onSignal(
          _large,
          jsonEncode({'t': 'this', 'k': ?token, 'now': 'aaaaaaaaaaaaaaaa'}),
        );
      }
      await Future<void>.delayed(const Duration(seconds: 2));
      expect(questions(_small), hasLength(1));
      expect(peer.asking, isFalse);
      expect(peer.theirCall, known);
      expect(peer.connection, same(connection));
      expect(up(small, large), isTrue);
    }),
  );

  test(
    'a participant that says it answers but never does keeps its call: asked a '
    'few times, then the late signal is dropped',
    () => inNetTime(() async {
      final (small, large) = await connected();
      final peer = small.peers[_large]!;
      final (known, connection) = (peer.theirCall, peer.connection);
      net.lose((from, to, message) => message['t'] == 'this', times: 100);
      await late(small, _large, 'again', '0123456789abcdef');
      await Future<void>.delayed(Call.whichEvery * (Call.whichTries + 1));
      expect(questions(_small), hasLength(Call.whichTries));
      expect(peer.asking, isFalse);
      expect(peer.theirCall, known);
      expect(peer.connection, same(connection));
      expect(up(small, large), isTrue);
    }),
  );

  test(
    'a copy before this one, which neither answers nor says it would, has its new '
    'call taken at once, as before',
    () => inNetTime(() async {
      // Its signals name its call but say nothing of answering.
      final small = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      await small.setParticipants([_small, _large]);
      final peer = small.peers[_large]!;
      await small.handleSignal(_large, {'t': 'again', 'c': 'first'});
      peer.upForTests();
      final made = net.madeBy(_small).length;
      await small.handleSignal(_large, {'t': 'again', 'c': 'second'});
      expect(questions(_small), isEmpty);
      expect(peer.theirCall, 'second');
      expect(peer.endedCalls, ['first']);
      expect(net.madeBy(_small).length, made + 1);
    }),
  );

  test(
    'a participant that comes back as a copy before this one, the room showing '
    'nothing, is connected to once its questions go unanswered',
    () => inNetTime(() async {
      final (small1, large) = await connected();
      await small1.stop();
      // Its new call says nothing of answering, and takes no question.
      net.older.add(_small);
      final small = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      final started = callClock();
      await small.setParticipants([_small, _large]);
      await until(
        () => up(small, large),
        what: 'connected to the older copy',
        limit: const Duration(seconds: 40),
      );
      // The questions' time, and the offer answered.
      expect(
        callClock().difference(started),
        lessThanOrEqualTo(
          Call.whichEvery * Call.whichTries + const Duration(seconds: 2),
        ),
      );
      expect(large.peers[_small]!.theirCall, callOf(small));
      // Its own try after its connection was not up in time finds it up.
      await Future<void>.delayed(const Duration(seconds: 6));
      expect(up(small, large), isTrue);
    }),
  );

  test(
    'a late signal of a copy before this one, after its device was updated, does '
    'not end the call of the update',
    () => inNetTime(() async {
      final (small, large) = await connected();
      final peer = large.peers[_small]!;
      final (known, connection) = (peer.theirCall, peer.connection);
      await late(large, _small, 'offer', '0123456789abcdef', answers: false);
      await Future<void>.delayed(Call.whichEvery * (Call.whichTries + 1));
      expect(up(small, large), isTrue);
      expect(peer.theirCall, known);
      expect(peer.connection, same(connection));
      expect(peer.endedCalls, contains('0123456789abcdef'));
    }),
  );

  test(
    'a late signal of a copy before this one that arrives before the updated '
    "device's new call is heard does not keep that call out",
    () => inNetTime(() async {
      final (small1, large) = await connected();
      await small1.stop();
      await late(large, _small, 'offer', '0123456789abcdef', answers: false);
      final small = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      await small.setParticipants([_small, _large]);
      await until(() => up(small, large), what: 'connected to the update');
      await Future<void>.delayed(Call.whichEvery * (Call.whichTries + 1));
      expect(up(small, large), isTrue);
      expect(large.peers[_small]!.theirCall, callOf(small));
    }),
  );

  for (final stopped in [true, false]) {
    test(
      'a signal of another call that waited its turn while '
      '${stopped ? 'the call stopped' : 'its sender left'} asks nothing',
      () => inNetTime(() async {
        final (_, large) = await connected();
        final asked = questions(_large).length;
        final waiting = late(large, _small, 'offer', '0123456789abcdef');
        if (stopped) {
          await large.stop();
        } else {
          await large.setParticipants([_large]);
        }
        await waiting;
        expect(questions(_large), hasLength(asked));
      }),
    );
  }

  test(
    "an answer of their new call to this side's offer, after their ended call's "
    'answer was set on its connection, has a connection made anew at once',
    () {
      // Connections take long to come up: the answer of their call before is set, and
      // the connection not up yet, when that call ends.
      net.uninstall();
      net = FakeNet(connecting: const Duration(seconds: 10))..install();
      return inNetTime(() async {
        final small = net.add(
          Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
        );
        final large1 = net.add(
          Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
        );
        await Future.wait([
          small.setParticipants([_small, _large]),
          large1.setParticipants([_small, _large]),
        ]);
        await until(
          () => net.madeBy(_small).single.remote != null,
          what: 'the answer set',
        );
        await large1.stop();
        // Their new call's asking is lost; a copy of this side's offer reaches it.
        net.lose(
          (from, to, message) => from == _large && message['t'] == 'again',
        );
        final large = net.add(
          Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
        );
        await large.setParticipants([_small, _large]);
        final offer = net.sent
            .lastWhere((s) => s.from == _small && s.type == 'offer')
            .message;
        await large.onSignal(_small, jsonEncode(offer));
        await Future<void>.delayed(const Duration(seconds: 1));
        expect(net.madeBy(_small), hasLength(2));
        await until(() => up(small, large), what: 'connected again');
        expect(small.peers[_large]!.theirCall, callOf(large));
      });
    },
  );

  test(
    "an answer of their ended call to this side's offer, arriving after their new "
    "call's answer to a copy of it, does not end the new call",
    () => inNetTime(() async {
      // Nothing of their call before is known here: its asking is lost, and its answer
      // held up on its way.
      net.lose(
        (from, to, message) => from == _large && message['t'] == 'again',
      );
      net.delay(
        (from, to, message) => from == _large && message['t'] == 'answer',
        const Duration(seconds: 3),
      );
      final small = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      final large1 = net.add(
        Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
      );
      await Future.wait([
        small.setParticipants([_small, _large]),
        large1.setParticipants([_small, _large]),
      ]);
      await until(
        () => net.sent.any((s) => s.from == _large && s.type == 'answer'),
        what: 'the answer sent',
      );
      await large1.stop();
      // Their new call's asking is lost; a copy of this side's offer reaches it, and its
      // answer arrives first.
      net.lose(
        (from, to, message) => from == _large && message['t'] == 'again',
      );
      final large = net.add(
        Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
      );
      await large.setParticipants([_small, _large]);
      final offer = net.sent
          .lastWhere((s) => s.from == _small && s.type == 'offer')
          .message;
      await large.onSignal(_small, jsonEncode(offer));
      await until(() => up(small, large), what: 'connected to the new call');
      // The late answer arrives, and well past it.
      await Future<void>.delayed(const Duration(seconds: 10));
      expect(up(small, large), isTrue);
      expect(small.peers[_large]!.theirCall, callOf(large));
    }),
  );

  test(
    'a late signal of an older copy taken when every answer was lost leaves '
    'their call of now to be asked about, not dropped',
    () => inNetTime(() async {
      final (small, large) = await connected();
      final now = callOf(small);
      // Every answer to the questions about it is lost.
      net.lose(
        (from, to, message) => message['t'] == 'this',
        times: Call.whichTries,
      );
      await late(large, _small, 'offer', '0123456789abcdef', answers: false);
      await Future<void>.delayed(
        Call.whichEvery * Call.whichTries + const Duration(seconds: 1),
      );
      // Taken: the connection with their call of now was let go of.
      expect(large.peers[_small]!.theirCall, '0123456789abcdef');
      // Theirs finds its connection failed, and offers anew.
      net.madeBy(_small).first.fail();
      final failed = callClock();
      await until(() => up(small, large), what: 'connected again');
      // Its try after the failure (4 s), a question and its answer, an offer and its
      // answer.
      expect(
        callClock().difference(failed),
        lessThan(const Duration(seconds: 7)),
      );
      expect(large.peers[_small]!.theirCall, now);
      expect(large.peers[_small]!.endedCalls, contains('0123456789abcdef'));
    }),
  );

  test(
    'someone back in the room with the call they left with, whose first signal is '
    'a late one of an older call, has the call they left with asked about',
    () => inNetTime(() async {
      final alone = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      await alone.setParticipants([_small, _large]);
      await alone.handleSignal(_large, {'t': 'again', 'c': 'first', 'w': 1});
      await alone.setParticipants([_small]);
      await alone.setParticipants([_small, _large]);
      final peer = alone.peers[_large]!;
      // A late asking of an older call comes first, then one of the call they have.
      await alone.handleSignal(_large, {'t': 'again', 'c': 'older', 'w': 1});
      await alone.handleSignal(_large, {'t': 'again', 'c': 'first', 'w': 1});
      expect(peer.endedCalls, isNot(contains('first')));
      expect(peer.asking, isTrue);
      final token = net.sent.lastWhere((s) => s.type == 'which').message['k'];
      await alone.handleSignal(_large, {
        't': 'this',
        'k': token,
        'now': 'first',
      });
      expect(peer.theirCall, 'first');
      expect(peer.endedCalls, contains('older'));
      // Its asking held, then handled: an offer made for their call.
      await Future<void>.delayed(const Duration(seconds: 1));
    }),
  );
}
