// The link between two devices in a call goes down a few seconds (a network change, a
// short outage) and comes back: the call comes back at once, without the two sides' tries
// undoing each other, and a whole network coming back tries each participant once.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/types.dart';

import 'call_net.dart';
import 'wait.dart';

// The smaller id offers, the larger asks for offers.
const _small = '2000000000000000';
const _large = 'd000000000000000';

void main() {
  final messenger =
      TestWidgetsFlutterBinding.ensureInitialized().defaultBinaryMessenger;
  var textures = 0;
  late FakeNet net;

  setUp(() {
    // The pictures of a participant, made ready as it comes.
    messenger.setMockMethodCallHandler(
      const MethodChannel('FlutterWebRTC.Method'),
      (call) async => call.method == 'createVideoRenderer'
          ? <String, dynamic>{'textureId': ++textures}
          : <String, dynamic>{},
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
    await until(
      () => small.peers[_large]!.connected && large.peers[_small]!.connected,
      what: 'connected',
    );
    return (small, large);
  }

  /// The offers the smaller side sent since [at]: one may have gone out as the link came
  /// back, made by a try while it was down; then none more is needed.
  int offersSince(DateTime at) => net.sent
      .where((s) => s.from == _small && s.type == 'offer' && s.at.isAfter(at))
      .length;

  /// The link goes down: the connection fails on both sides, and the core says so to
  /// each. It stays down past the tries after the failure (a few seconds on), which go
  /// nowhere.
  Future<void> outage(Call small, Call large) async {
    final cut = DateTime.now();
    net.cut(_small, _large);
    small.memberGone(_large);
    large.memberGone(_small);
    // The tries after the failure go while it is down, and then none for a while: what
    // brings the call back is the link coming back, not a try that happens to follow it.
    await until(
      () => offersSince(cut) > 0,
      what: 'a try while the link is down',
    );
    await until(
      () => net.sent.every(
        (s) =>
            s.type != 'offer' ||
            DateTime.now().difference(s.at) > const Duration(seconds: 2),
      ),
      what: 'no try for a while',
    );
  }

  bool up(Call small, Call large) =>
      small.peers[_large]!.connected && large.peers[_small]!.connected;

  test('a member that reconnects after a short outage is back within the first '
      'try', () async {
    final (small, large) = await connected();
    await outage(small, large);
    net.mend(_small, _large);
    final back = DateTime.now();
    small.memberBack(_large);
    large.memberBack(_small);
    await until(() => up(small, large), what: 'connected again');
    // One offer and its answer: about what a first connection takes, not the seconds a
    // connection that never came up waits before it is tried again.
    expect(
      DateTime.now().difference(back),
      lessThan(const Duration(seconds: 2)),
    );
    expect(offersSince(back), lessThanOrEqualTo(1));
  });

  for (final asksFirst in [true, false]) {
    test('two sides trying at once end with one connection'
        '${asksFirst ? ', the asking arriving first' : ''}', () async {
      final (small, large) = await connected();
      await outage(small, large);
      final answers = net.madeBy(_large).length;
      net.mend(_small, _large);
      final back = DateTime.now();
      if (asksFirst) {
        // The larger side hears of the link first, and its asking reaches the other
        // before the core tells the other of the link.
        large.memberBack(_small);
        await until(
          () => net.sent.any(
            (s) => s.from == _large && s.type == 'again' && s.at.isAfter(back),
          ),
          what: 'the asking',
        );
        await Future<void>.delayed(net.latency * 2);
        small.memberBack(_large);
      } else {
        small.memberBack(_large);
        large.memberBack(_small);
      }
      await until(() => up(small, large), what: 'connected again');
      // A moment more: a second offer would replace the connection that came up.
      await Future<void>.delayed(const Duration(seconds: 1));
      expect(up(small, large), isTrue);
      expect(offersSince(back), lessThanOrEqualTo(1), reason: 'offers sent');
      expect(net.madeBy(_large).length, answers + 1, reason: 'answers made');
    });
  }

  test(
    'ten members reconnecting together do not each try more than once',
    () async {
      // This device's network changed: every link went down and comes back at once. Half
      // the others have smaller ids (this device asks them for an offer), half larger (it
      // offers to them).
      const me = '8000000000000000';
      final others = [
        for (final first in '01234abcde'.split('')) '${first}f00000000000000',
      ];
      final call = net.add(
        Call(group: 'g', me: me)..bridgeForTests = fakeBridge,
      );
      await call.setParticipants([me, ...others]);
      for (final id in others) {
        call.peers[id]!.connected = false;
        call.memberGone(id);
      }
      final back = DateTime.now();
      // The core may tell of each twice (a link, then a better way to it).
      for (var i = 0; i < 2; i++) {
        for (final id in others) {
          call.memberBack(id);
        }
      }
      await Future<void>.delayed(const Duration(seconds: 1));
      for (final id in others) {
        final tries = net.sent.where(
          (s) =>
              s.to == id &&
              s.at.isAfter(back) &&
              (s.type == 'offer' || s.type == 'again'),
        );
        expect(tries.length, 1, reason: 'tries to $id');
      }
    },
  );

  test(
    "the core telling of the link reaches the call, a direct call's too",
    () async {
      // A direct call's room is not among the groups: what reaches its call is told the same
      // way, by the call's group.
      for (final group in ['g', 'direct call']) {
        final call = net.add(
          Call(group: group, me: _small)..bridgeForTests = fakeBridge,
        );
        await call.setParticipants([_small, _large]);
        final events = StreamController<UiEvent>();
        AppState('/nonexistent/fogata-outage-test')
          ..callState.call = call
          ..listenTo(events.stream);
        final made = net.madeBy(_small).length;
        UiEvent event(UiEventKind kind) => UiEvent(
          kind: kind,
          group: group,
          device: _large,
          path: UiPath.direct,
          received: 0,
          total: 0,
        );
        events
          ..add(event(UiEventKind.memberDisconnected))
          ..add(event(UiEventKind.memberConnected));
        await until(
          () => net.madeBy(_small).length > made,
          what: 'tried again in $group',
        );
        await events.close();
        net.calls.remove(_small);
      }
    },
  );
}
