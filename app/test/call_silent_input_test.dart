// The silent input (Linux) that WebRTC records from while the microphone is off and a
// shared screen's sound goes out: a call that ends, or a microphone that comes on, while
// it is being chosen never leaves WebRTC recording it, in this call or the next.

import 'dart:async';
import 'dart:io' show pid;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/host.dart';

class _Track implements MediaStreamTrack {
  _Track(this.kind);

  @override
  final String kind;

  @override
  Future<void> stop() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// A shared screen captured with its sound.
class _Screen implements MediaStream {
  final _tracks = [_Track('video'), _Track('audio')];

  @override
  List<MediaStreamTrack> getTracks() => _tracks;

  @override
  List<MediaStreamTrack> getAudioTracks() =>
      _tracks.where((t) => t.kind == 'audio').toList();

  @override
  List<MediaStreamTrack> getVideoTracks() =>
      _tracks.where((t) => t.kind == 'video').toList();

  @override
  Future<void> dispose() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

/// The microphone's track, which a mute disables.
class _MicTrack extends _Track {
  _MicTrack() : super('audio');

  @override
  bool enabled = true;
}

/// The microphone's stream.
class _Mic implements MediaStream {
  final track = _MicTrack();

  @override
  List<MediaStreamTrack> getTracks() => [track];

  @override
  List<MediaStreamTrack> getAudioTracks() => [track];

  @override
  List<MediaStreamTrack> getVideoTracks() => const [];

  @override
  Future<void> dispose() async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

const _channel = MethodChannel('FlutterWebRTC.Method');
final _silenceId = 'silence-$pid';

/// The voice engine's side, as the calls below reach it: what was asked, in order, and
/// the steps a test holds until it lets them go.
class _Engine {
  final asked = <String>[];

  /// Held until completed: making the silent input, choosing an input.
  Completer<void>? holdSilence;
  Completer<void>? holdSelect;

  /// Held until completed: choosing an input other than the silent one.
  Completer<void>? holdMic;

  /// Held until completed: the list of devices.
  Completer<void>? holdSources;

  /// Whether the silent input exists.
  bool silence = false;

  Future<Object?> handle(MethodCall call) async {
    final args = call.arguments as Map<Object?, Object?>?;
    switch (call.method) {
      case 'getSources':
        await holdSources?.future;
        return {
          'sources': [
            if (silence)
              {
                'deviceId': _silenceId,
                'label': '$silenceLabel ($pid)',
                'kind': 'audioinput',
              },
            {
              'deviceId': 'mic-1',
              'label': 'Desk microphone',
              'kind': 'audioinput',
            },
            {
              'deviceId': 'speaker-1',
              'label': 'Desk speakers',
              'kind': 'audiooutput',
            },
          ],
        };
      case 'fogataSilentInput':
        final on = args!['on'] == true;
        asked.add('silence ${on ? 'on' : 'off'}');
        if (on) await holdSilence?.future;
        silence = on;
        return true;
      case 'selectAudioInput':
        final id = args!['deviceId'] as String;
        asked.add('select ${id == _silenceId ? 'silence' : id}');
        if (id == _silenceId) {
          await holdSelect?.future;
        } else {
          await holdMic?.future;
        }
        return null;
      case 'selectAudioOutput':
        asked.add('output ${args!['deviceId']}');
        return null;
      case 'fogataScreenSound':
        asked.add('screen sound ${args!['on'] == true ? 'on' : 'off'}');
        await holdSilence?.future;
        return true;
    }
    return null;
  }

  /// What WebRTC records from once everything asked is done.
  String? get recording {
    final selected = asked.where((a) => a.startsWith('select ')).lastOrNull;
    return selected?.substring('select '.length);
  }
}

/// A call sharing a screen with its sound, the microphone off.
Call _sharing() {
  final call = Call(group: 'g', me: 'me', mic: false)..sharing = true;
  call.holdForTests(streams: [null, null, _Screen()]);
  return call;
}

/// Lets the steps run until [done] says so.
Future<void> _until(bool Function() done) async {
  for (var i = 0; i < 200 && !done(); i++) {
    await Future<void>.delayed(Duration.zero);
  }
  expect(done(), isTrue);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _Engine engine;
  setUp(() {
    engine = _Engine();
    letGoLimit = const Duration(seconds: 2);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, engine.handle);
  });
  tearDown(() {
    letGoLimit = const Duration(seconds: 5);
    pinnedHost = HostSystem.linux;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, null);
  });

  test('a microphone that comes on while the silent input is made leaves it '
      'unchosen', () async {
    engine.holdSilence = Completer();
    final call = _sharing();
    final quiet = call.setShareSound(true);
    await _until(() => engine.asked.contains('silence on'));
    // On, as the system answers; not through setMic, whose own turn would mend it.
    call.mic = true;
    engine.holdSilence!.complete();
    await quiet;
    expect(engine.asked, isNot(contains('select silence')));
    expect(engine.asked.last, 'silence off');
    expect(engine.silence, isFalse);
  });

  test('a microphone that comes on while the silent input is chosen goes back '
      'to the microphone', () async {
    engine.holdSelect = Completer();
    final call = _sharing();
    final quiet = call.setShareSound(true);
    await _until(() => engine.asked.contains('select silence'));
    call.mic = true;
    engine.holdSelect!.complete();
    await quiet;
    expect(engine.recording, 'mic-1');
    expect(engine.silence, isFalse);
  });

  test('a call that ends while the silent input is chosen ends once it is let '
      'go of, recording from the microphone', () async {
    engine.holdSelect = Completer();
    final call = _sharing();
    unawaited(call.setShareSound(true));
    await _until(() => engine.asked.contains('select silence'));
    var stopped = false;
    final stop = call.stop().then((_) => stopped = true);
    for (var i = 0; i < 20; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    // Still choosing: the stop waits for it rather than finding nothing to undo.
    expect(stopped, isFalse);
    engine.holdSelect!.complete();
    await stop.timeout(const Duration(seconds: 5));
    expect(engine.recording, 'mic-1');
    expect(engine.silence, isFalse);
    // Nothing more is asked once the call has ended.
    final after = engine.asked.length;
    await pumpEventQueue();
    expect(engine.asked.length, after);
  });

  test('a silent input gone with the sound server is made and chosen '
      'again', () async {
    final call = _sharing();
    await call.setShareSound(true);
    expect(engine.recording, 'silence');
    // PipeWire restarted: the node went with its connection, and the list read as the
    // devices change no longer has it.
    engine.silence = false;
    engine.asked.clear();
    call.takeDevices(await navigator.mediaDevices.enumerateDevices());
    await _until(() => engine.asked.contains('select silence'));
    expect(engine.asked, ['silence on', 'select silence']);
    expect(engine.silence, isTrue);
  });

  // Another microphone chosen, muted, while the silent input is being chosen: taken
  // after that choice, it is kept for when the silent input is left, and WebRTC never
  // records the microphone meanwhile.
  test('a microphone chosen while the silent input is chosen records '
      'nothing', () async {
    engine.holdSelect = Completer();
    final call = _sharing();
    try {
      final quiet = call.setShareSound(true);
      await _until(() => engine.asked.contains('select silence'));
      // The system's default, as chosen in the settings.
      final use = call.useInput(null);
      for (var i = 0; i < 20; i++) {
        await Future<void>.delayed(Duration.zero);
      }
      engine.holdSelect!.complete();
      await quiet;
      await use;
      expect(engine.recording, 'silence');
    } finally {
      await call.stop();
    }
    expect(engine.recording, 'mic-1');
  });

  test('a call with the system default records from a real input, never a '
      'silent one left by the call before', () async {
    engine.silence = true;
    await Call(group: 'g', me: 'me', mic: false).chooseInputForTests();
    expect(engine.asked, ['select mic-1']);

    engine.asked.clear();
    await Call(
      group: 'g',
      me: 'me',
      mic: false,
      input: 'usb-2',
    ).chooseInputForTests();
    expect(engine.asked, ['select usb-2']);
  });

  // A call's start chooses its input outside the input turn, with no time limit: it
  // waits for a slow list of devices and still chooses the system's default, never
  // leaving the device given last.
  test('a call\'s start waits for a slow list of devices', () async {
    engine.holdSources = Completer();
    final call = Call(group: 'g', me: 'me', mic: false);
    final choosing = call.chooseInputForTests();
    await Future<void>.delayed(const Duration(milliseconds: 200));
    engine.holdSources!.complete();
    await choosing;
    expect(engine.asked, ['select mic-1']);
  });

  test('a call with the system defaults chooses them by their own ids on every '
      'system: the voice engine keeps the devices it was given last, from a '
      'microphone test or a call before', () async {
    for (final system in [
      HostSystem.linux,
      HostSystem.macos,
      HostSystem.windows,
    ]) {
      pinnedHost = system;
      engine.asked.clear();
      final call = Call(group: 'g', me: 'me', mic: false);
      await call.chooseOutputForTests();
      await call.chooseInputForTests();
      expect(engine.asked, [
        'output speaker-1',
        'select mic-1',
      ], reason: '$system');
    }
  });

  test('a call that ends while its screen sound starts (macOS) stops taking '
      'it', () async {
    pinnedHost = HostSystem.macos;
    engine.holdSilence = Completer();
    final call = Call(group: 'g', me: 'me', mic: false)..sharing = true;
    final share = call.setShareSound(true);
    await _until(() => engine.asked.contains('screen sound on'));
    await call.stop();
    engine.holdSilence!.complete();
    await share;
    expect(engine.asked.last, 'screen sound off');
  });

  test(
    'an unmute while a mute waits for the silent input goes out at once',
    () async {
      engine.holdSelect = Completer();
      final mic = _Mic();
      final call = Call(group: 'g', me: 'me', mic: true)..sharing = true;
      call.holdForTests(streams: [mic, null, _Screen()]);
      await call.setShareSound(true);
      unawaited(call.setMic(false));
      await _until(() => engine.asked.contains('select silence'));
      expect(mic.track.enabled, isFalse);
      // The sound server does not answer the choice of the silent input: the unmute
      // does not wait for it.
      final on = call.setMic(true);
      expect(mic.track.enabled, isTrue);
      expect(call.mic, isTrue);
      engine.holdSelect!.complete();
      await on.timeout(const Duration(seconds: 5));
      expect(mic.track.enabled, isTrue);
      expect(engine.recording, 'mic-1');
      await call.stop();
    },
  );

  test('a mute, an unmute and a mute again while the silent input is chosen '
      'end muted, on the silent input', () async {
    engine.holdSelect = Completer();
    final mic = _Mic();
    final call = Call(group: 'g', me: 'me', mic: true)..sharing = true;
    call.holdForTests(streams: [mic, null, _Screen()]);
    await call.setShareSound(true);
    unawaited(call.setMic(false));
    await _until(() => engine.asked.contains('select silence'));
    unawaited(call.setMic(true));
    expect(mic.track.enabled, isTrue);
    unawaited(call.setMic(false));
    // Muted again at once, before anything held is let go of.
    expect(mic.track.enabled, isFalse);
    expect(call.mic, isFalse);
    engine.holdSelect!.complete();
    await call.voiceSettledForTests().timeout(const Duration(seconds: 5));
    expect(mic.track.enabled, isFalse);
    expect(engine.recording, 'silence');
    await call.stop();
  });

  test(
    'of the inputs chosen while one is being chosen, only the last is carried '
    'out',
    () async {
      engine.holdMic = Completer();
      final call = Call(group: 'g', me: 'me', mic: true);
      final first = call.useInput('mic-2');
      await _until(() => engine.asked.contains('select mic-2'));
      final second = call.useInput('mic-3');
      final third = call.useInput('mic-4');
      engine.holdMic!.complete();
      await Future.wait([first, second, third])
          .timeout(const Duration(seconds: 5));
      expect(engine.asked.where((a) => a.startsWith('select ')), [
        'select mic-2',
        'select mic-4',
      ]);
      expect(call.input, 'mic-4');
    },
  );
}
