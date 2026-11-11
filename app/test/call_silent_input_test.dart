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

const _channel = MethodChannel('FlutterWebRTC.Method');
final _silenceId = 'silence-$pid';

/// The voice engine's side, as the calls below reach it: what was asked, in order, and
/// the steps a test holds until it lets them go.
class _Engine {
  final asked = <String>[];

  /// Held until completed: making the silent input, choosing an input.
  Completer<void>? holdSilence;
  Completer<void>? holdSelect;

  /// Whether the silent input exists.
  bool silence = false;

  Future<Object?> handle(MethodCall call) async {
    final args = call.arguments as Map<Object?, Object?>?;
    switch (call.method) {
      case 'getSources':
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
        if (id == _silenceId) await holdSelect?.future;
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
}
