import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';

/// What a step of letting go does: ends, fails, or never ends.
enum Way { ends, fails, hangs }

/// A step that goes [way]; a hung one ends once [unhang] completes.
Future<void> _go(Way way, Completer<void> unhang) async {
  switch (way) {
    case Way.ends:
      return;
    case Way.fails:
      throw StateError('a native error');
    case Way.hangs:
      await unhang.future;
  }
}

class _Track implements MediaStreamTrack {
  _Track(this.way, this.unhang);
  final Way way;
  final Completer<void> unhang;
  bool stopped = false;

  @override
  Future<void> stop() async {
    await _go(way, unhang);
    stopped = true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Stream implements MediaStream {
  _Stream(this.tracks, this.way, this.unhang, {this.tracksFail = false});
  final List<_Track> tracks;
  final Way way;
  final Completer<void> unhang;
  final bool tracksFail;
  bool disposed = false;

  @override
  List<MediaStreamTrack> getTracks() {
    if (tracksFail) throw StateError('no tracks');
    return tracks;
  }

  @override
  Future<void> dispose() async {
    await _go(way, unhang);
    disposed = true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Connection implements RTCPeerConnection {
  _Connection(this.way, this.unhang);
  final Way way;
  final Completer<void> unhang;
  bool closed = false;

  @override
  Future<void> close() async {
    await _go(way, unhang);
    closed = true;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A connection that takes [time] to close, noting in [order] when it is done.
class _Slow implements RTCPeerConnection {
  _Slow(this.name, this.time, this.order);
  final String name;
  final Duration time;
  final List<String> order;

  @override
  Future<void> close() async {
    await Future<void>.delayed(time);
    order.add(name);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A connection that counts its closes.
class _Counted implements RTCPeerConnection {
  _Counted(this.closed);
  final void Function() closed;

  @override
  Future<void> close() async => closed();

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A participant whose letting go never ends until [unhang] completes.
class _Stuck extends CallPeer {
  _Stuck(super.id, this.unhang, this.started);
  final Completer<void> unhang;
  final List<String> started;

  @override
  Future<void> dispose() {
    started.add(id);
    return unhang.future;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() => letGoLimit = const Duration(milliseconds: 50));
  tearDown(() => letGoLimit = const Duration(seconds: 5));

  test(
    'the voice engine closes after every step of letting go of a peer',
    () async {
      // Each step is within its limit, but together they are over one.
      letGoLimit = const Duration(milliseconds: 100);
      final order = <String>[];
      const time = Duration(milliseconds: 80);
      final call = Call(group: 'g', me: 'me', mic: false);
      call.peers['ana'] = CallPeer('ana')
        ..connection = _Slow('connection', time, order)
        ..soundConnection = _Slow('sound', time, order);
      call.holdForTests(engine: _Slow('engine', Duration.zero, order));
      await call.stop().timeout(const Duration(seconds: 5));
      expect(order, ['connection', 'sound', 'engine']);
    },
  );

  test('participants are let go of together, within the time of one', () async {
    letGoLimit = const Duration(milliseconds: 100);
    final unhang = Completer<void>();
    final call = Call(group: 'g', me: 'me', mic: false);
    final connections = <_Connection>[];
    for (var i = 0; i < 6; i++) {
      final connection = _Connection(Way.hangs, unhang);
      final sound = _Connection(Way.hangs, unhang);
      connections.add(connection);
      call.peers['p$i'] = CallPeer('p$i')
        ..connection = connection
        ..soundConnection = sound;
    }
    final engine = _Connection(Way.ends, Completer());
    call.holdForTests(engine: engine);
    final took = Stopwatch()..start();
    await call.stop().timeout(const Duration(seconds: 5));
    took.stop();
    // Two steps of each hang: one after the other, they took 12 limits.
    expect(took.elapsed, lessThan(letGoLimit * 4));
    expect(call.peers, isEmpty);
    // Each one's steps were given up on in time, so the voice engine stops; the
    // connections still close once they can.
    expect(engine.closed, isTrue);
    unhang.complete();
    await pumpEventQueue();
    expect(connections.every((c) => c.closed), isTrue);
  });

  test('participants whose letting go never ends hold the stop for one\'s limit, and the '
      'voice engine until they end', () async {
    letGoLimit = const Duration(milliseconds: 100);
    final unhang = Completer<void>();
    final started = <String>[];
    final call = Call(group: 'g', me: 'me', mic: false);
    for (var i = 0; i < 3; i++) {
      call.peers['p$i'] = _Stuck('p$i', unhang, started);
    }
    final engine = _Connection(Way.ends, Completer());
    call.holdForTests(engine: engine);
    final took = Stopwatch()..start();
    await call.stop().timeout(const Duration(seconds: 5));
    took.stop();
    final one = letGoLimit * CallPeer.disposeSteps;
    expect(started, ['p0', 'p1', 'p2']);
    expect(took.elapsed, greaterThanOrEqualTo(one));
    expect(took.elapsed, lessThan(one * 1.5));
    expect(engine.closed, isFalse);
    unhang.complete();
    await pumpEventQueue();
    expect(engine.closed, isTrue);
  });

  test('participants that leave together are let go of together', () async {
    letGoLimit = const Duration(milliseconds: 100);
    final unhang = Completer<void>();
    final call = Call(group: 'g', me: 'me', mic: false);
    for (var i = 0; i < 4; i++) {
      call.peers['p$i'] = CallPeer('p$i')
        ..connection = _Connection(Way.hangs, unhang)
        ..soundConnection = _Connection(Way.hangs, unhang);
    }
    call.peers['ana'] = CallPeer('ana')
      ..connection = _Connection(Way.ends, Completer());
    final took = Stopwatch()..start();
    await call
        .setParticipants(['me', 'ana'])
        .timeout(const Duration(seconds: 5));
    took.stop();
    // Two steps of each hang: one after the other, they took 8 limits.
    expect(took.elapsed, lessThan(letGoLimit * 4));
    expect(call.peers.keys, ['ana']);
    unhang.complete();
    await call.stop().timeout(const Duration(seconds: 5));
  });

  test('participants still being let go of after they left hold the voice engine too', () async {
    letGoLimit = const Duration(milliseconds: 100);
    final unhang = Completer<void>();
    final started = <String>[];
    final call = Call(group: 'g', me: 'me', mic: false);
    for (var i = 0; i < 3; i++) {
      call.peers['p$i'] = _Stuck('p$i', unhang, started);
    }
    final engine = _Connection(Way.ends, Completer());
    call.holdForTests(engine: engine);
    // All three leave, and the call ends while they are let go of.
    final leaving = call.setParticipants(['me']);
    await pumpEventQueue();
    expect(started, ['p0', 'p1', 'p2']);
    expect(call.peers, isEmpty);
    final took = Stopwatch()..start();
    await call.stop().timeout(const Duration(seconds: 5));
    took.stop();
    // Given up on within one's limit, and the voice engine waits for them.
    expect(took.elapsed, lessThan(letGoLimit * CallPeer.disposeSteps * 1.5));
    expect(engine.closed, isFalse);
    unhang.complete();
    await leaving.timeout(const Duration(seconds: 5));
    await pumpEventQueue();
    expect(engine.closed, isTrue);
  });

  test('a device opening when the call ends closes the voice engine only after a track '
      'still stopping', () async {
    final unhang = Completer<void>();
    final mic = _Track(Way.hangs, unhang);
    final engine = _Connection(Way.ends, Completer());
    final call = Call(group: 'g', me: 'me', mic: false);
    call.holdForTests(
      streams: [
        _Stream([mic], Way.ends, unhang),
      ],
      engine: engine,
    );
    // A camera is being opened (the system asking, say) as the call ends.
    final asked = Completer<void>();
    final opening = call.openingForTests(() => asked.future);
    await call.stop().timeout(const Duration(seconds: 5));
    // The microphone's track did not stop in its time: still stopping.
    expect(mic.stopped, isFalse);
    expect(engine.closed, isFalse);
    // The camera's opening ends, and returns at once, while the voice engine still waits
    // for that track.
    asked.complete();
    await opening.timeout(const Duration(seconds: 1));
    await pumpEventQueue();
    expect(
      engine.closed,
      isFalse,
      reason: 'closed under a track still stopping',
    );
    // The track stops at last: then the voice engine closes, once.
    unhang.complete();
    await pumpEventQueue();
    expect(mic.stopped, isTrue);
    expect(engine.closed, isTrue);
  });

  test('two devices opening when the call ends close the voice engine once, after the '
      'last', () async {
    var closes = 0;
    final engine = _Counted(() => closes++);
    final call = Call(group: 'g', me: 'me', mic: false);
    call.holdForTests(engine: engine);
    final first = Completer<void>();
    final second = Completer<void>();
    final a = call.openingForTests(() => first.future);
    final b = call.openingForTests(() => second.future);
    await call.stop().timeout(const Duration(seconds: 5));
    expect(closes, 0);
    first.complete();
    await a;
    await pumpEventQueue();
    expect(closes, 0);
    second.complete();
    await b;
    await pumpEventQueue();
    expect(closes, 1);
  });

  const stages = [
    'the tracks',
    'a track',
    'a stream',
    'a connection',
    'the voice engine',
  ];
  for (final stage in stages) {
    for (final way in [Way.fails, if (stage != 'the tracks') Way.hangs]) {
      test('everything else is let go of when $stage ${way.name}', () async {
        final unhang = Completer<void>();
        Way at(String s) => s == stage ? way : Way.ends;
        final mic = _Track(at('a track'), unhang);
        final camera = _Track(Way.ends, unhang);
        final screen = _Track(Way.ends, unhang);
        final streams = [
          _Stream(
            [mic],
            Way.ends,
            unhang,
            tracksFail: stage == 'the tracks' && way == Way.fails,
          ),
          _Stream([camera], at('a stream'), unhang),
          _Stream([screen], Way.ends, unhang),
        ];
        final ana = _Connection(at('a connection'), unhang);
        final bea = _Connection(Way.ends, unhang);
        final engine = _Connection(at('the voice engine'), unhang);
        final call = Call(group: 'g', me: 'me', mic: false);
        call.peers['ana'] = CallPeer('ana')..connection = ana;
        call.peers['bea'] = CallPeer('bea')..connection = bea;
        call.holdForTests(streams: streams, engine: engine);

        final stopping = call.stop();
        // Asked again (the room left as the window closes, say): the same letting go.
        expect(identical(call.stop(), stopping), isTrue);
        await stopping.timeout(const Duration(seconds: 5));

        expect(camera.stopped && screen.stopped, isTrue);
        // Only the one that failed or still hangs is not stopped.
        expect(mic.stopped, stage != 'a track' && stage != 'the tracks');
        expect(streams[0].disposed && streams[2].disposed, isTrue);
        expect(bea.closed, isTrue);
        expect(call.peers, isEmpty);
        final tracksHung =
            way == Way.hangs && (stage == 'a track' || stage == 'a stream');
        if (stage != 'the voice engine') {
          // The voice engine stops only once this device's tracks are let go of.
          expect(engine.closed, !tracksHung);
        }
        unhang.complete();
        await pumpEventQueue();
        if (stage != 'the voice engine' || way == Way.hangs) {
          expect(engine.closed, isTrue);
        }
        // A new call starts and ends in turn.
        final next = Call(group: 'g', me: 'me', mic: false);
        final again = _Connection(Way.ends, Completer());
        next.peers['ana'] = CallPeer('ana')..connection = again;
        await next.stop().timeout(const Duration(seconds: 5));
        expect(again.closed, isTrue);
      });
    }
  }
}
