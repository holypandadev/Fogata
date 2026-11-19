// Signals that take seconds each (a computer so loaded that every signal waits seconds to
// be sent and handled): a negotiation that goes on, however slowly, is not thrown away
// and begun anew while its offer or answer is still on its way, and one whose offer was
// lost is still tried again as soon as before.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/call.dart';

import 'call_net.dart';
import 'fake_time.dart';
import 'wait.dart';

// The smaller id offers, the larger asks for offers.
const _small = '2600000000000000';
const _large = 'd600000000000000';

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;
  late FakeNet net;

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

  /// Two calls that come into the room together, every signal between them taking
  /// [latency]: how long until both are connected, what was sent, and the connections
  /// each side made.
  Future<({Duration took, int offers, int numbers, int agains, int made})>
  connect({SignalMatch? lost}) async {
    final small = net.add(
      Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
    );
    final large = net.add(
      Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
    );
    if (lost != null) net.lose(lost);
    final start = callClock();
    await Future.wait([
      small.setParticipants([_small, _large]),
      large.setParticipants([_small, _large]),
    ]);
    await until(
      () => up(small, large),
      what: 'connected',
      limit: const Duration(minutes: 3),
    );
    final took = callClock().difference(start);
    final offers = [
      for (final s in net.sent)
        if (s.from == _small && s.type == 'offer') s.message['n'],
    ];
    return (
      took: took,
      offers: offers.length,
      numbers: offers.toSet().length,
      agains: net.sent
          .where((s) => s.from == _large && s.type == 'again')
          .length,
      made: net.madeBy(_small).length + net.madeBy(_large).length,
    );
  }

  for (final seconds in [0.5, 2, 4, 6]) {
    final latency = Duration(milliseconds: (seconds * 1000).round());
    test(
      'signals of ${seconds}s each: one negotiation, connected within its own time',
      () async {
        net = FakeNet(latency: latency)..install();
        await inCallTime(
          () async {
            final r = await connect();
            // ignore: avoid_print
            print(
              'latency $seconds s: connected after ${r.took.inMilliseconds} ms, '
              '${r.offers} offers sent (${r.numbers} negotiations), '
              '${r.agains} askings, ${r.made} connections made',
            );
            // An offer and its answer, each with its candidates gathered, and the
            // connection coming up; a second for what the calls do between.
            final own =
                latency * 2 +
                net.gathering * 2 +
                net.connecting +
                const Duration(seconds: 1);
            expect(r.took, lessThanOrEqualTo(own));
            // One negotiation: copies of its offer at most, no connection made anew.
            expect(r.numbers, 1);
            expect(r.made, 2);
          },
          stop: net.stop,
          within: const Duration(minutes: 4),
        );
      },
    );
  }

  test('an answer that takes seconds and a connection slow to come up: the '
      'negotiation is not given up on while it goes on', () async {
    const latency = Duration(seconds: 4);
    // Up 13 s after both descriptions are set: past the 15 s from the offer.
    net = FakeNet(latency: latency, connecting: const Duration(seconds: 13))
      ..install();
    await inCallTime(() async {
      final r = await connect();
      expect(
        r.took,
        lessThanOrEqualTo(
          latency * 2 +
              net.gathering * 2 +
              net.connecting +
              const Duration(seconds: 1),
        ),
      );
      expect(r.numbers, 1);
      expect(r.made, 2);
      // The other side's own wait ran out meanwhile (it cannot tell when its answer
      // arrived): its asking for a new offer finds this one up, and changes nothing.
      await Future<void>.delayed(const Duration(seconds: 10));
      expect(net.madeBy(_small), hasLength(1));
      expect(net.madeBy(_large), hasLength(1));
    }, stop: net.stop);
  });

  for (final (what, from, type) in [
    ('offer', _small, 'offer'),
    ('answer', _large, 'answer'),
  ]) {
    test('a lost $what is tried again as soon as before', () async {
      net = FakeNet()..install();
      await inCallTime(() async {
        final r = await connect(
          lost: (sender, to, message) => sender == from && message['t'] == type,
        );
        // The first try, 5 s on, and what a negotiation takes.
        expect(
          r.took,
          lessThanOrEqualTo(
            const Duration(seconds: 5) +
                net.latency * 2 +
                net.gathering * 2 +
                net.connecting +
                const Duration(seconds: 1),
          ),
        );
        // Its copy did it, on the connections made for it.
        expect(r.offers, 2);
        expect(r.numbers, 1);
        expect(r.made, 2);
      }, stop: net.stop);
    });
  }

  for (final answersCopies in [true, false]) {
    test(
      'a first offer with no answer goes again '
      '${answersCopies ? 'as it is' : 'anew to a copy before this one'}',
      () async {
        net = FakeNet()..install();
        await inCallTime(() async {
          // The other device's call is not here: nothing answers.
          final small = net.add(
            Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
          );
          await small.setParticipants([_small, _large]);
          // Its asking, as a copy before this one sends it (naming its call, saying
          // nothing of copies) or as this one does.
          await small.handleSignal(_large, {
            't': 'again',
            'c': 'first',
            if (answersCopies) 'w': 1,
          });
          await Future<void>.delayed(const Duration(seconds: 6));
          final numbers = [
            for (final s in net.sent)
              if (s.type == 'offer') s.message['n'],
          ];
          expect(numbers, hasLength(2));
          expect(numbers.toSet(), hasLength(answersCopies ? 1 : 2));
          expect(net.madeBy(_small), hasLength(answersCopies ? 1 : 2));
        }, stop: net.stop);
      },
    );
  }

  test('an offer that arrives after its copy changes nothing', () async {
    net = FakeNet()..install();
    await inCallTime(() async {
      // Held up past the first try, whose copy arrives first and is answered.
      net.delay(
        (sender, to, message) => sender == _small && message['t'] == 'offer',
        const Duration(seconds: 6),
      );
      final r = await connect();
      expect(r.numbers, 1);
      await Future<void>.delayed(const Duration(seconds: 10));
      expect(net.madeBy(_small), hasLength(1));
      expect(net.madeBy(_large), hasLength(1));
      expect(net.madeBy(_large).single.closed, isFalse);
    }, stop: net.stop);
  });

  test('a connection that never comes up is made anew within twice its time '
      'at most', () async {
    net = FakeNet(connecting: const Duration(hours: 1))..install();
    await inCallTime(() async {
      final small = net.add(
        Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
      );
      final large = net.add(
        Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
      );
      final start = callClock();
      await Future.wait([
        small.setParticipants([_small, _large]),
        large.setParticipants([_small, _large]),
      ]);
      bool anew() =>
          net.sent
              .where((s) => s.from == _small && s.type == 'offer')
              .map((s) => s.message['n'])
              .toSet()
              .length >
          1;
      await until(anew, what: 'a new offer', limit: const Duration(minutes: 1));
      // Its answer came at once: given up on [Call.connectWithin] after it, and offered
      // anew after the pause of a first try again (4 s), as before.
      expect(
        callClock().difference(start),
        lessThanOrEqualTo(Call.connectWithin + const Duration(seconds: 4 + 1)),
      );
    }, stop: net.stop);
  });

  test(
    'a copy of an offer that reaches an end that failed is answered on a new '
    'one',
    () async {
      net = FakeNet()..install();
      await inCallTime(() async {
        final small = net.add(
          Call(group: 'g', me: _small)..bridgeForTests = fakeBridge,
        );
        final large = net.add(
          Call(group: 'g', me: _large)..bridgeForTests = fakeBridge,
        );
        // The answer is lost, and the end that gave it fails.
        net.lose((sender, to, message) => message['t'] == 'answer');
        final start = callClock();
        await Future.wait([
          small.setParticipants([_small, _large]),
          large.setParticipants([_small, _large]),
        ]);
        await until(
          () =>
              net.madeBy(_large).isNotEmpty &&
              net.madeBy(_large).first.local != null,
          what: 'the answer made',
        );
        net.madeBy(_large).first.fail();
        await until(
          () => up(small, large),
          what: 'connected',
          limit: const Duration(seconds: 40),
        );
        // The first try's copy, 5 s on, answered on a new end.
        expect(
          callClock().difference(start),
          lessThanOrEqualTo(const Duration(seconds: 7)),
        );
        expect(net.madeBy(_large), hasLength(2));
        // The other side's asking after its failure may still be on its way.
        await Future<void>.delayed(const Duration(seconds: 6));
      }, stop: net.stop);
    },
  );

  test('an answer that arrives twice is refused the second time, as WebRTC '
      'refuses it, and changes nothing', () async {
    net = FakeNet()..install();
    await inCallTime(() async {
      net.duplicate(
        (sender, to, message) => message['t'] == 'answer',
        const Duration(milliseconds: 500),
      );
      final r = await connect();
      await Future<void>.delayed(const Duration(seconds: 2));
      expect(r.made, 2);
      expect(net.madeBy(_small), hasLength(1));
      expect(net.madeBy(_large), hasLength(1));
      expect(net.madeBy(_small).single.closed, isFalse);
      expect(up(net.calls[_small]!, net.calls[_large]!), isTrue);
    }, stop: net.stop);
  });

  test(
    'a late copy of an earlier first offer, after a later one came up, leaves '
    'the connection as it is',
    () async {
      net = FakeNet()..install();
      await inCallTime(() async {
        // The first offer arrives once more, 8 s on.
        net.duplicate(
          (sender, to, message) => sender == _small && message['t'] == 'offer',
          const Duration(seconds: 8),
        );
        await connect();
        final small = net.calls[_small]!;
        final large = net.calls[_large]!;
        // Both ends fail; the smaller side offers anew, and that comes up.
        net.madeBy(_small).single.fail();
        net.madeBy(_large).single.fail();
        await until(() => up(small, large), what: 'connected again');
        final connection = large.peers[_small]!.connection;
        await Future<void>.delayed(const Duration(seconds: 10));
        expect(up(small, large), isTrue);
        expect(large.peers[_small]!.connection, same(connection));
        expect(net.madeBy(_large), hasLength(2));
      }, stop: net.stop);
    },
  );

  test("a participant's first offer after it was made anew is taken", () async {
    net = FakeNet()..install();
    await inCallTime(() async {
      await connect();
      final small = net.calls[_small]!;
      final large = net.calls[_large]!;
      int lastOffer() =>
          net.sent.lastWhere((s) => s.type == 'offer').message['n'] as int;
      final before = lastOffer();
      // The smaller side's room showed the other leave and come back; the other's did
      // not, and keeps its participant with what it took from it.
      await small.setParticipants([_small]);
      await small.setParticipants([_small, _large]);
      await until(() => up(small, large), what: 'connected again');
      expect(lastOffer(), greaterThan(before));
      expect(net.madeBy(_large), hasLength(2));
    }, stop: net.stop);
  });
}
