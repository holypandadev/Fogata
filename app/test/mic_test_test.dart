// The settings' microphone test, with a microphone that is never real: it opens only when
// asked, shows how loud it is, plays it back only when asked, closes when its part goes
// away, the window is hidden or a call starts, and says what the calls say when the
// microphone cannot be used.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/mic_test.dart';
import 'package:fogata/src/shown.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// A microphone that is never real: its level is set by the test.
class FakeMic implements MicLoop {
  FakeMic(this.input, this.output, this.heard, this.noise);
  final String? input;
  final String? output;
  bool heard;
  final bool noise;
  double now = 0;
  bool stopped = false;

  @override
  Future<double> level() async => now;

  @override
  set hear(bool on) => heard = on;

  @override
  Future<void> stop() async => stopped = true;
}

(MicTest, List<FakeMic>) _fakeTest() {
  final mics = <FakeMic>[];
  final test = MicTest(
    open:
        ({
          String? input,
          String? output,
          required bool hear,
          required bool noiseSuppression,
        }) async {
          final mic = FakeMic(input, output, hear, noiseSuppression);
          mics.add(mic);
          return mic;
        },
  );
  return (test, mics);
}

AppState _state() => fakeRoom(people: 2)..callState.call = null;

Future<void> _pump(
  WidgetTester tester,
  AppState state,
  MicTest test, {
  bool hasMic = true,
}) async {
  tester.view.physicalSize = const Size(800, 600);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Center(
            child: SizedBox(
              width: 560,
              child: TestMic(loaded: true, hasMic: hasMic, test: test),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

FgButton _button(WidgetTester tester) =>
    tester.widget<FgButton>(find.byKey(const ValueKey('test-mic-button')));

double _fill(WidgetTester tester) => tester
    .widget<AnimatedFractionallySizedBox>(
      find.byKey(const ValueKey('mic-level-fill')),
    )
    .widthFactor!;

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'the microphone opens only on Test microphone, and its level shows',
    (tester) async {
      final (test, mics) = _fakeTest();
      final state = _state()
        ..prefs.audioInput = 'mic-2'
        ..prefs.audioOutput = 'out-1'
        ..prefs.noiseSuppression = true;
      await _pump(tester, state, test);
      expect(mics, isEmpty);
      expect(
        find.text('Nothing from the test is sent to anyone.'),
        findsOneWidget,
      );
      expect(_fill(tester), 0);
      await tester.tap(find.text('Test microphone'));
      await tester.pump();
      final mic = mics.single;
      expect((mic.input, mic.output, mic.noise), ('mic-2', 'out-1', true));
      // Not played back unless asked.
      expect(mic.heard, isFalse);
      mic.now = 0.1;
      await tester.pump(MicTest.every);
      await tester.pump(MicTest.every);
      expect(test.level.value, 0.1);
      expect(_fill(tester), closeTo(meterShare(0.1), 1e-9));
      expect(_fill(tester), greaterThan(0.5));
      expect(find.text('Stop test'), findsOneWidget);
      await tester.tap(find.text('Stop test'));
      await tester.pump();
      expect(mic.stopped, isTrue);
      expect(test.level.value, 0);
      expect(_fill(tester), 0);
      expect(find.text('Test microphone'), findsOneWidget);
    },
  );

  testWidgets(
    'hearing it back is off until turned on, and back off next time',
    (tester) async {
      final (test, mics) = _fakeTest();
      final semantics = tester.ensureSemantics();
      await _pump(tester, _state(), test);
      expect(find.bySemanticsLabel(RegExp('Hear myself off')), findsOneWidget);
      expect(find.textContaining('Use headphones'), findsOneWidget);
      await tester.tap(find.text('Test microphone'));
      await tester.pump();
      await tester.tap(find.text('Hear myself'));
      await tester.pump();
      expect(mics.single.heard, isTrue);
      expect(find.bySemanticsLabel(RegExp('Hear myself on')), findsOneWidget);
      await tester.tap(find.text('Hear myself'));
      await tester.pump();
      expect(mics.single.heard, isFalse);
      await tester.tap(find.text('Hear myself'));
      await tester.pump();
      // Leaving the part closes the microphone, and forgets the playing back.
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
      expect(mics.single.stopped, isTrue);
      expect(test.on, isFalse);
      expect(test.hearing, isFalse);
      semantics.dispose();
    },
  );

  testWidgets('a hidden window closes the microphone', (tester) async {
    final (test, mics) = _fakeTest();
    await _pump(tester, _state(), test);
    await tester.tap(find.text('Test microphone'));
    await tester.pump();
    windowShown.value = false;
    addTearDown(() => windowShown.value = true);
    await tester.pump();
    expect(mics.single.stopped, isTrue);
    expect(test.on, isFalse);
  });

  testWidgets('choosing another microphone goes on with that one', (
    tester,
  ) async {
    final (test, mics) = _fakeTest();
    final state = _state()..prefs.audioInput = 'mic-1';
    await _pump(tester, state, test);
    await tester.tap(find.text('Test microphone'));
    await tester.pump();
    state.prefs.audioInput = 'mic-2';
    state.notifyListeners();
    await tester.pump();
    await tester.pump();
    expect(mics.map((m) => m.input), ['mic-1', 'mic-2']);
    expect(mics.first.stopped, isTrue);
    expect(mics.last.stopped, isFalse);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('no microphone: the button is off and says why', (tester) async {
    final (test, mics) = _fakeTest();
    await _pump(tester, _state(), test, hasMic: false);
    expect(find.text('No microphone is connected.'), findsOneWidget);
    expect(_button(tester).onTap, isNull);
    await tester.tap(find.text('Test microphone'), warnIfMissed: false);
    await tester.pump();
    expect(mics, isEmpty);
  });

  testWidgets('in a call the call has the microphone and the test waits', (
    tester,
  ) async {
    final (test, mics) = _fakeTest();
    await _pump(tester, fakeRoom(people: 2), test);
    expect(find.text('The microphone is in use in your call.'), findsOneWidget);
    expect(_button(tester).onTap, isNull);
    expect(mics, isEmpty);
  });

  testWidgets('a microphone that cannot be used says so as a call does', (
    tester,
  ) async {
    final test = MicTest(
      open: ({
        String? input,
        String? output,
        required bool hear,
        required bool noiseSuppression,
      }) async => throw StateError('NotAllowedError'),
    );
    await _pump(tester, _state(), test);
    await tester.tap(find.text('Test microphone'));
    await tester.pump();
    expect(
      find.textContaining('Could not use the microphone.'),
      findsOneWidget,
    );
    expect(test.failed, isTrue);
    expect(find.text('Test microphone'), findsOneWidget);
  });

  test('a stop while the microphone opens closes it once open', () async {
    final gate = Completer<void>();
    final mics = <FakeMic>[];
    final test = MicTest(
      open:
          ({
            String? input,
            String? output,
            required bool hear,
            required bool noiseSuppression,
          }) async {
            await gate.future;
            final mic = FakeMic(input, output, hear, noiseSuppression);
            mics.add(mic);
            return mic;
          },
    );
    unawaited(test.start());
    expect(test.opening, isTrue);
    var stopped = false;
    final stopping = test.stop().then((_) => stopped = true);
    await Future<void>.delayed(Duration.zero);
    expect(stopped, isFalse);
    gate.complete();
    await stopping;
    expect(mics.single.stopped, isTrue);
    expect(test.on, isFalse);
  });

  /// A test whose microphones open only once [gate] completes.
  (MicTest, List<FakeMic>) gated(Completer<void> gate) {
    final mics = <FakeMic>[];
    final test = MicTest(
      open:
          ({
            String? input,
            String? output,
            required bool hear,
            required bool noiseSuppression,
          }) async {
            await gate.future;
            final mic = FakeMic(input, output, hear, noiseSuppression);
            mics.add(mic);
            return mic;
          },
    );
    return (test, mics);
  }

  test('a start while a call takes the microphone is refused', () async {
    final gate = Completer<void>();
    final (test, mics) = gated(gate);
    unawaited(test.start());
    // A call starts: the test stops, and while it stops someone presses Test again.
    final blocking = test.block();
    expect(test.blocked, isTrue);
    await test.start();
    expect(test.on, isFalse);
    gate.complete();
    await blocking;
    // Only the first opened, and it was closed at once.
    expect(mics, hasLength(1));
    expect(mics.single.stopped, isTrue);
    await test.start();
    expect(test.on, isFalse);
    // Once the call is over, it can start again.
    test.unblock();
    await test.start();
    expect(test.on, isTrue);
    await test.stop();
  });

  test('two starts at once open one microphone', () async {
    final gate = Completer<void>();
    final (test, mics) = gated(gate);
    final first = test.start();
    final second = test.start();
    gate.complete();
    await Future.wait([first, second]);
    expect(mics, hasLength(1));
    await test.stop();
    expect(mics.single.stopped, isTrue);
  });

  test('a microphone whose opening hangs does not hold up the call', () async {
    final before = MicTest.stopWait;
    MicTest.stopWait = const Duration(milliseconds: 100);
    addTearDown(() => MicTest.stopWait = before);
    final gate = Completer<void>();
    final (test, mics) = gated(gate);
    unawaited(test.start());
    final watch = Stopwatch()..start();
    await test.block();
    expect(watch.elapsed, lessThan(const Duration(seconds: 2)));
    // It opens late: it is closed at once.
    gate.complete();
    await Future<void>.delayed(Duration.zero);
    await Future<void>.delayed(Duration.zero);
    expect(mics.single.stopped, isTrue);
    expect(test.on, isFalse);
  });

  test(
    'a call starting gives the microphone up first, even one still opening',
    () async {
      TestWidgetsFlutterBinding.ensureInitialized();
      final before = (micTest.open, MicTest.stopWait);
      final gate = Completer<void>();
      final (gatedTest, mics) = gated(gate);
      micTest.open = gatedTest.open;
      MicTest.stopWait = const Duration(milliseconds: 100);
      addTearDown(() {
        micTest.open = before.$1;
        MicTest.stopWait = before.$2;
        micTest.unblock();
      });
      unawaited(micTest.start());
      expect(micTest.opening, isTrue);
      final state = fakeRoom(people: 2)..callState.call = null;
      // The call itself cannot start here (no WebRTC, no core): what matters is what comes
      // before it.
      try {
        await state.startCallForTests('g').timeout(const Duration(seconds: 5));
      } catch (_) {}
      expect(micTest.blocked, isTrue);
      expect(micTest.on, isFalse);
      await micTest.start();
      expect(micTest.on, isFalse);
      gate.complete();
      await Future<void>.delayed(Duration.zero);
      await Future<void>.delayed(Duration.zero);
      expect(mics.single.stopped, isTrue);
      state.callState.call = null;
    },
  );

  test('the meter shows from -60 dB up, so a voice fills most of it', () {
    expect(meterShare(0), 0);
    expect(meterShare(1), 1);
    expect(meterShare(0.001), closeTo(0, 1e-9));
    expect(meterShare(0.0001), 0);
    expect(meterShare(0.03), closeTo(0.49, 0.01));
  });

  group('alone in a call', () {
    test(
      'the own level comes from a loop of its own, made once per track',
      () async {
        var opened = 0;
        final level = AloneLevel(
          open: (stream, track) async {
            opened++;
            throw StateError('no WebRTC here');
          },
        );
        // No microphone: nothing is opened.
        expect(await level.read(null), 0);
        expect(opened, 0);
        final stream = _FakeStream([_FakeTrack('t1')]);
        expect(await level.read(stream), 0);
        expect(await level.read(stream), 0);
        // A loop that could not be made is not tried again for the same track.
        expect(opened, 1);
        expect(await level.read(_FakeStream([_FakeTrack('t2')])), 0);
        expect(opened, 2);
        await level.close();
      },
    );

    test(
      'another microphone: the old loop goes before the new one is made',
      () async {
        final steps = <String>[];
        final level = AloneLevel(
          open: (stream, track) async {
            steps.add('open ${track.id}');
            return _FakeLevel(track.id!, steps);
          },
        );
        expect(await level.read(_FakeStream([_FakeTrack('t1')])), 0.25);
        expect(await level.read(_FakeStream([_FakeTrack('t1')])), 0.25);
        expect(await level.read(_FakeStream([_FakeTrack('t2')])), 0.25);
        // The microphone off: its loop goes.
        expect(await level.read(null), 0);
        expect(steps, ['open t1', 'close t1', 'open t2', 'close t2']);
      },
    );
  });
}

class _FakeLevel implements TrackLevel {
  _FakeLevel(this.trackId, this.steps);
  @override
  final String trackId;
  final List<String> steps;
  @override
  Future<double> level() async => 0.25;
  @override
  Future<void> close() async => steps.add('close $trackId');
}

class _FakeTrack extends Fake implements MediaStreamTrack {
  _FakeTrack(this.id);
  @override
  final String id;
}

class _FakeStream extends Fake implements MediaStream {
  _FakeStream(this.tracks);
  final List<MediaStreamTrack> tracks;
  @override
  List<MediaStreamTrack> getAudioTracks() => tracks;
}
