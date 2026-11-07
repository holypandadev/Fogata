// The sound that goes with a shared window.
//
// macOS: for each window asked for (and once for a whole screen), takes its sound as a
// call does, sends it from one connection to another inside this copy (both made by the
// screen sound's own factory, which opens no microphone or speakers), and logs what the
// sound's source measured and what went out, then which applications the capture took
// and how loud they were (the plugin's own lines, "FogataScreenSound: ..."). Nothing is
// played: with the computer silent, every level must be 0. FOGATA_SOUND_WINDOWS lists
// window numbers (comma-separated); without it, the windows listed for sharing are taken.
// With FOGATA_SOUND_WINDOW_NAME and FOGATA_SOUND_FLAG instead, it checks that a shared
// window brings its own application's sound and no other's, as on Windows below: it takes
// the sound of the window of that name while another program plays (nothing may come),
// then creates the file FOGATA_SOUND_FLAG, on which the window's application starts
// playing (that must come). tool/mac_sound_test.sh sets this up and runs it.
//
// Windows: shares the window titled FOGATA_SOUND_WINDOW_NAME with its sound, as a call
// does, while another program plays: nothing may come. Then it creates the file
// FOGATA_SOUND_FLAG, on which the window's own program starts playing: that must come.
// Both programs play into a virtual audio cable, set up by whoever starts this.
//
// FOGATA_CALL is the folder for sound.log and sound-result.txt.

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _dir = _env['FOGATA_CALL'] ?? Directory.systemTemp.path;

void _log(String line) {
  stderr.writeln('sound test: $line');
  File('$_dir/sound.log').writeAsStringSync(
    '${DateTime.now().toIso8601String()} $line\n',
    mode: FileMode.append,
  );
}

Future<bool> _take(bool on, String? window) async =>
    await WebRTC.invokeMethod<bool, dynamic>('fogataScreenSound', {
      'on': on,
      'window': ?window,
    }) ??
    false;

/// Takes the sound of [window] (a whole screen's when null) for [seconds] and logs what
/// was measured; returns the loudest level seen at the source.
Future<double> _measure(String? window, {int seconds = 6}) async {
  final what = window == null ? 'a whole screen' : 'window $window';
  if (!await _take(true, window)) {
    _log('$what: the sound could not be taken');
    return -1;
  }
  final config = {'fogataScreenSound': true, 'sdpSemantics': 'unified-plan'};
  final from = await createPeerConnection(config);
  final to = await createPeerConnection(config);
  from.onIceCandidate = (c) => to.addCandidate(c);
  to.onIceCandidate = (c) => from.addCandidate(c);
  final up = Completer<void>();
  from.onConnectionState = (state) {
    if (state == RTCPeerConnectionState.RTCPeerConnectionStateConnected &&
        !up.isCompleted) {
      up.complete();
    }
  };
  final offer = await from.createOffer({});
  await from.setLocalDescription(offer);
  await to.setRemoteDescription(offer);
  final answer = await to.createAnswer({});
  await to.setLocalDescription(answer);
  await from.setRemoteDescription(answer);
  await up.future.timeout(const Duration(seconds: 15));
  var loudest = 0.0;
  for (var i = 0; i < seconds; i++) {
    await Future<void>.delayed(const Duration(seconds: 1));
    for (final report in await from.getStats()) {
      final v = report.values;
      if (report.type == 'media-source' && v['kind'] == 'audio') {
        final level = (v['audioLevel'] as num? ?? 0).toDouble();
        if (level > loudest) loudest = level;
        if (i == seconds - 1) {
          _log(
            '$what: level at the source ${v['audioLevel']}, '
            'energy ${v['totalAudioEnergy']}',
          );
        }
      } else if (report.type == 'outbound-rtp' && i == seconds - 1) {
        _log('$what: ${v['packetsSent']} packets sent');
      }
    }
  }
  await from.close();
  await to.close();
  await _take(false, null);
  // The plugin says what the capture took once it has stopped.
  await Future<void>.delayed(const Duration(seconds: 1));
  return loudest;
}

/// The loudest level the source of [track] gives for [seconds], sent from one connection
/// to another inside this copy; what arrives is not played.
Future<double> _loudest(
  MediaStreamTrack track,
  MediaStream stream,
  int seconds,
) async {
  final from = await createPeerConnection({'sdpSemantics': 'unified-plan'});
  final to = await createPeerConnection({'sdpSemantics': 'unified-plan'});
  from.onIceCandidate = (c) => to.addCandidate(c);
  to.onIceCandidate = (c) => from.addCandidate(c);
  to.onTrack = (event) => Helper.setVolume(0, event.track);
  await from.addTrack(track, stream);
  final up = Completer<void>();
  from.onConnectionState = (state) {
    if (state == RTCPeerConnectionState.RTCPeerConnectionStateConnected &&
        !up.isCompleted) {
      up.complete();
    }
  };
  final offer = await from.createOffer({});
  await from.setLocalDescription(offer);
  await to.setRemoteDescription(offer);
  final answer = await to.createAnswer({});
  await to.setLocalDescription(answer);
  await from.setRemoteDescription(answer);
  await up.future.timeout(const Duration(seconds: 15));
  var loudest = 0.0;
  for (var i = 0; i < seconds; i++) {
    await Future<void>.delayed(const Duration(seconds: 1));
    for (final report in await from.getStats()) {
      final v = report.values;
      if (report.type == 'media-source' && v['kind'] == 'audio') {
        final level = (v['audioLevel'] as num? ?? 0).toDouble();
        if (level > loudest) loudest = level;
      }
    }
  }
  await from.close();
  await to.close();
  return loudest;
}

Future<void> _windows() async {
  final name = _env['FOGATA_SOUND_WINDOW_NAME']!;
  final flag = File(_env['FOGATA_SOUND_FLAG']!);
  final sources = await desktopCapturer.getSources(types: [SourceType.Window]);
  final window = sources.firstWhere((s) => s.name == name);
  _log('window "$name": ${window.id}');
  final stream = await navigator.mediaDevices.getDisplayMedia({
    'audio': true,
    'video': {
      'deviceId': {'exact': window.id},
      'mandatory': {'frameRate': 5.0},
    },
  });
  final track = stream.getAudioTracks().first;
  final others = await _loudest(track, stream, 8);
  _log('another program playing: loudest $others');
  flag.writeAsStringSync('play');
  await Future<void>.delayed(const Duration(seconds: 2));
  final own = await _loudest(track, stream, 8);
  _log('the window\'s program playing too: loudest $own');
  for (final t in stream.getTracks()) {
    await t.stop();
  }
  await stream.dispose();
  final ok = others < 0.001 && own > 0.01;
  File(
    '$_dir/sound-result.txt',
  ).writeAsStringSync(ok ? 'passed\n' : 'failed: others $others, own $own\n');
}

/// macOS: the window named [name] brings its application's sound and no other's (see the
/// top of this file).
Future<void> _macWindow(String name) async {
  final flag = File(_env['FOGATA_SOUND_FLAG']!);
  final sources = await desktopCapturer.getSources(types: [SourceType.Window]);
  final window = sources.firstWhere((s) => s.name.contains(name));
  _log('window "${window.name}": ${window.id}');
  final others = await _measure(window.id, seconds: 8);
  _log('another program playing: loudest $others');
  flag.writeAsStringSync('play');
  await Future<void>.delayed(const Duration(seconds: 2));
  final own = await _measure(window.id, seconds: 8);
  _log('the window\'s application playing too: loudest $own');
  final ok = others >= 0 && others < 0.001 && own > 0.01;
  File(
    '$_dir/sound-result.txt',
  ).writeAsStringSync(ok ? 'passed\n' : 'failed: others $others, own $own\n');
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('on Windows a shared window brings only its program\'s sound', (
    tester,
  ) async {
    if (!Platform.isWindows) return;
    await tester.pumpWidget(
      const MaterialApp(home: Center(child: Text('Fogata sound test'))),
    );
    try {
      await tester.runAsync(_windows);
    } catch (e, trace) {
      File('$_dir/sound-result.txt').writeAsStringSync('failed: $e\n$trace\n');
    }
  });

  testWidgets('a shared window takes only its application\'s sound', (
    tester,
  ) async {
    if (!Platform.isMacOS) return;
    await tester.pumpWidget(
      const MaterialApp(home: Center(child: Text('Fogata sound test'))),
    );
    final result = File('$_dir/sound-result.txt');
    final name = _env['FOGATA_SOUND_WINDOW_NAME'];
    if (name != null) {
      try {
        await tester.runAsync(() => _macWindow(name));
      } catch (e, trace) {
        result.writeAsStringSync('failed: $e\n$trace\n');
      }
      return;
    }
    try {
      final asked = _env['FOGATA_SOUND_WINDOWS'];
      final windows = asked != null
          ? asked.split(',').where((w) => w.isNotEmpty).toList()
          : [
              for (final s in await desktopCapturer.getSources(
                types: [SourceType.Window],
              ))
                s.id,
            ];
      _log('windows: ${windows.join(', ')}');
      final levels = <String, double>{};
      for (final window in [...windows, null]) {
        levels[window ?? 'screen'] =
            await tester.runAsync(() => _measure(window)) as double;
      }
      _log('loudest levels: $levels');
      final heard = levels.entries.where((e) => e.value != 0);
      result.writeAsStringSync(
        heard.isEmpty ? 'passed\n' : 'failed: sound from ${heard.join(', ')}\n',
      );
    } catch (e, trace) {
      result.writeAsStringSync('failed: $e\n$trace\n');
    }
  });
}
