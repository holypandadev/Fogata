// A call between copies of the app, on one computer or on several. Build once with
//
//   flutter build linux --debug -t integration_test/call_test.dart
//
// (or `flutter build macos` / `flutter build windows`) and start one copy per role:
// FOGATA_ROLE=a to h, FOGATA_PEERS the number of copies (2 when left out), and a
// different FOGATA_DATA_DIR each. Copy a writes a friend link for each of the others in
// turn to FOGATA_CALL/link-<role>.txt, and each other copy reads its own from its
// FOGATA_CALL folder; on several computers, carry the files over as they appear. Copy a
// then makes a group with everyone, and from there the copies take turns through the
// room's chat. Each copy writes <role>.log and <role>-result.txt ("passed" or the reason
// it failed) to its FOGATA_CALL folder. The waits leave time for devices on different
// networks. FOGATA_TEST_QUALITY=1 checks instead how each person's connection shows while
// the network around the copies changes, and then ends: see _connectionQuality.
//
// Without more, the copies only connect. FOGATA_TEST_MIC=1 adds sound with virtual devices
// only: an input called FogataMic that carries a tone, a silent one called FogataQuiet and
// an output called FogataTest. FOGATA_TEST_MIC=off enters with the microphone off, with
// the same virtual devices. FOGATA_TEST_MIC=system uses the computer's own microphone and
// speakers instead, where that is allowed: the system's defaults, or with FOGATA_TEST_INPUT
// and FOGATA_TEST_OUTPUT the input and the output whose names contain them, chosen as the
// settings choose a device (a computer whose default microphone is silent, as a phone's
// virtual one with no phone). With a tone on copy a, the others also see it
// mute and change microphone. FOGATA_TEST_CAMERA=1 turns the camera on.
// FOGATA_TEST_SCREEN=1 shares the first screen, best on a virtual display since the others
// receive what it shows; FOGATA_TEST_SCREEN=window shares the app's own window. Each
// start and stop of the share logs the longest the platform thread took to answer
// meanwhile (a share whose start holds it freezes the window), and
// FOGATA_TEST_SCREEN_AGAIN=<n> has that copy share and stop n times first, then, on
// Windows, ask for two captures with sound at once; see _shareAgain. With
// FOGATA_SHOTS set to a folder, a copy that watches a screen saves pictures of it there:
// its controls shown, the connection's quality, and the controls hidden after a while.
// FOGATA_TEST_WATCH=<seconds> measures instead how a shared screen and the voice come
// through (for a network made slow around the copies): the copy with FOGATA_TEST_SCREEN
// shares, the others watch that long, and each logs every 2 s what goes out and what
// arrives, then a summary; see _watchScreen. FOGATA_TEST_TYPING=1 checks instead who shows
// as typing under the room's chat while the others write, and then ends: see _typing.
// FOGATA_TEST_NOISE=<seconds> has copy a (or the copy FOGATA_TEST_NOISE_ROLE names) turn
// noise suppression off and on in the middle of the call instead; see _noiseSteps.
// FOGATA_TEST_CAMERA_STEPS=<seconds> measures instead what each camera sends while half
// the copies leave the room and come back; see _cameraSteps. On Linux without a camera,
// FOGATA_TEST_CAMERA=1 gives each copy a made-up one. FOGATA_TEST_CAMERA_START=<seconds>
// measures instead how soon copy a's camera, turned on after a while of voice only, reaches
// the others at the size and bitrate it goes out at; see _cameraStart.
// FOGATA_TEST_HOSTILE=1 (two copies, with FOGATA_TEST_CAMERA=1 on Linux without a camera)
// has copy b act instead as a modified copy would against copy a; see _hostile.
// FOGATA_TEST_HOSTILE=giant (copy b started with FOGATA_TEST_CAMERA_GIANT=<w>x<h>) has its
// camera send now and then a picture far larger than any copy sends; see _hostileGiant.
// FOGATA_TEST_HOSTILE=offers has copy b offer a new connection as fast as it may; see
// _hostileOffers.
// FOGATA_TEST_HOSTILE=unanswered (copy b started with FOGATA_TEST_UNANSWERED=<file>, and
// maybe FOGATA_TEST_CAMERA_GIANT) has copy b stop answering copy a's connectivity checks
// and then send its camera; see _hostileUnanswered.
// FOGATA_TEST_HOSTILE=stuck (two copies, with FOGATA_TEST_CAMERA=1) has copy b's screens
// stop for FOGATA_TEST_STUCK seconds while copy a turns its camera on and off; see
// _hostileStuck.
// FOGATA_TEST_MEASURE=<seconds> measures instead the call as it is, with every copy's voice
// and, with FOGATA_TEST_CAMERA=1, its camera, and with FOGATA_TEST_SCREEN on one copy, its
// screen watched by all the others; see _measureCall. FOGATA_TEST_MIC_CASE checks instead
// one thing of the microphone (muted, switch, unplug, late, alone, broken, quit, devices,
// noise)
// or of the call (rejoin, echo, restart);
// see _micCase. Both are run by tools/call-lab. FOGATA_TEST_MIC_LATE=<seconds> has the
// microphone open only that long after the room does, so the others connect first.
// FOGATA_TEST_MIC_CASE=rejoin (two copies, both with the microphone on, on one computer or
// on two) has each copy in turn leave the room and enter it again, and both measure how
// soon the call is back; FOGATA_TEST_REJOIN_TIMES sets how many times; see _rejoin.

import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:flutter_webrtc/flutter_webrtc.dart'
    show
        RTCPeerConnection,
        RTCPeerConnectionState,
        RTCRtpMediaType,
        RTCRtpTransceiverInit,
        RTCVideoRenderer,
        SourceType,
        StatsReport,
        TransceiverDirection,
        WebRTC,
        DesktopCapturerSource,
        createPeerConnection,
        desktopCapturer,
        getRtpReceiverCapabilities,
        navigator;
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/l10n.dart' show l10n;
import 'package:fogata/src/mic_test.dart' show micTest;
import 'package:fogata/src/quality.dart';
import 'package:fogata/src/rust/api/app.dart' as api;
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/widgets.dart';
import 'package:fogata/src/window.dart' show quitApplication;
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;

/// Saves a picture of the window as `name`.png in FOGATA_SHOTS, if set.
Future<void> _shot(WidgetTester tester, String name) async {
  final dir = _env['FOGATA_SHOTS'];
  if (dir == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(Offset.zero & view.size);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$dir/$name.png').writeAsBytes(data!.buffer.asUint8List());
}

final _role = _env['FOGATA_ROLE'] ?? 'a';
final _dir = _env['FOGATA_CALL'] ?? Directory.systemTemp.path;
final _copies = int.tryParse(_env['FOGATA_PEERS'] ?? '') ?? 2;
final _mic = _env['FOGATA_TEST_MIC'];
final _camera = _env['FOGATA_TEST_CAMERA'] == '1';
final _share = _env['FOGATA_TEST_SCREEN'];

const _names = {
  'a': 'Ana',
  'b': 'Bea',
  'c': 'Carla',
  'd': 'Dani',
  'e': 'Eva',
  'f': 'Fran',
  'g': 'Gabi',
  'h': 'Hugo',
};

/// The virtual microphones, with FOGATA_TEST_MIC=1.
String? _tone;
String? _quiet;

void _log(String line) => File('$_dir/$_role.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

Future<void> _wait(WidgetTester tester, Duration d) async {
  final end = DateTime.now().add(d);
  while (DateTime.now().isBefore(end)) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await tester.pump();
  }
}

Future<void> _until(
  WidgetTester tester,
  String what,
  bool Function() done, {
  int seconds = 120,
}) async {
  final end = DateTime.now().add(Duration(seconds: seconds));
  while (!done()) {
    if (DateTime.now().isAfter(end)) throw StateError('not in time: $what');
    await _wait(tester, const Duration(milliseconds: 200));
  }
  _log(what);
}

/// Presses Escape. The physical key is named here: left out, flutter_test looks it up by
/// the keys' debug names, which only a debug build has, so a profile build (the one the
/// calls are tried with) would stop there.
Future<void> _escape(WidgetTester tester) => tester.sendKeyEvent(
  LogicalKeyboardKey.escape,
  physicalKey: PhysicalKeyboardKey.escape,
);

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _until(tester, 'shown: $finder', () => finder.evaluate().isNotEmpty);
  await tester.ensureVisible(finder.first);
  await tester.tap(finder.first);
  await _wait(tester, const Duration(milliseconds: 300));
}

AppState _state(WidgetTester tester) =>
    AppScope.read(tester.element(find.byType(Shell)));

/// Tells the other copies a step through the room's chat.
Future<void> _say(AppState state, String word) async {
  await state.sendChat('test: $word');
  _log('said: $word');
}

/// Another copy in the call: its device id and name.
typedef _Other = ({String id, String name});

/// Which of [words] each other copy said first in the room's chat.
Map<_Other, String> _said(AppState state, List<String> words) {
  final said = <_Other, String>{};
  for (final UiChatLine line
      in state.roomState.room?.chat ?? const <UiChatLine>[]) {
    final who = (id: line.sender, name: line.nick);
    if (line.mine || said.containsKey(who)) continue;
    for (final word in words) {
      if (line.text == 'test: $word') said[who] = word;
    }
  }
  return said;
}

/// Waits until every other copy said one of [words].
Future<Map<_Other, String>> _hear(
  WidgetTester tester,
  AppState state,
  List<String> words, {
  int seconds = 180,
}) async {
  await _until(
    tester,
    'heard from everyone: ${words.join(' or ')}',
    () => _said(state, words).length >= _copies - 1,
    seconds: seconds,
  );
  return _said(state, words);
}

/// Waits until one other copy said [word].
Future<void> _hearFrom(
  WidgetTester tester,
  AppState state,
  _Other other,
  String word,
) => _until(
  tester,
  'heard from ${other.name}: $word',
  () => _said(state, [word]).containsKey(other),
  seconds: 180,
);

/// The connection to another copy.
CallPeer _peer(AppState state, _Other other) =>
    state.callState.call!.peers[other.id] ??
    (throw StateError('no connection to ${other.name}'));

/// A received count from the other copies so far, or from one: packets of audio, frames
/// of video.
Future<int> _received(
  AppState state,
  String kind,
  String field, {
  CallPeer? only,
}) async {
  var total = 0;
  for (final CallPeer peer in [...?state.callState.call?.peers.values]) {
    if (only != null && peer != only) continue;
    for (final report
        in await peer.connection?.getStats() ?? const <StatsReport>[]) {
      if (report.type == 'inbound-rtp' && report.values['kind'] == kind) {
        total += (report.values[field] as num? ?? 0).toInt();
      }
    }
  }
  return total;
}

/// Copy a (or the one FOGATA_TEST_NOISE_ROLE names) turns noise suppression off, on, off
/// and on, [seconds] each time, without leaving the call, and logs the moment each change
/// was asked for and done (microseconds since the epoch); whatever runs the copies feeds
/// its microphone and records what the others play. The others log, every second, how
/// much of its voice they had to make up.
Future<void> _noiseSteps(
  WidgetTester tester,
  AppState state,
  int seconds,
) async {
  if (_role == (_env['FOGATA_TEST_NOISE_ROLE'] ?? 'a')) {
    for (final on in [false, true, false, true]) {
      final asked = DateTime.now().microsecondsSinceEpoch;
      await state.setNoiseSuppression(on);
      _log(
        'noise suppression ${on ? 'on' : 'off'} '
        'at ${DateTime.now().microsecondsSinceEpoch} (asked at $asked)',
      );
      await _wait(tester, Duration(seconds: seconds));
    }
    return;
  }
  final end = DateTime.now().add(Duration(seconds: 4 * seconds + 10));
  var concealed = await _received(state, 'audio', 'concealedSamples');
  var samples = await _received(state, 'audio', 'totalSamplesReceived');
  while (DateTime.now().isBefore(end) && _said(state, ['done']).isEmpty) {
    await _wait(tester, const Duration(seconds: 1));
    final c = await _received(state, 'audio', 'concealedSamples');
    final t = await _received(state, 'audio', 'totalSamplesReceived');
    _log(
      'voice made up: ${c - concealed} of ${t - samples} samples '
      'at ${DateTime.now().microsecondsSinceEpoch}',
    );
    concealed = c;
    samples = t;
  }
}

/// Logs how the call is doing with each other copy: round trip, losses, jitter, video sizes
/// and freezes.
Future<void> _quality(AppState state, String when) async {
  String ms(Object? seconds) =>
      seconds is num ? '${(seconds * 1000).round()} ms' : '?';
  for (final CallPeer peer in [...?state.callState.call?.peers.values]) {
    final parts = <String>[];
    final reports = await peer.connection?.getStats() ?? const <StatsReport>[];
    final codecs = {
      for (final r in reports)
        if (r.type == 'codec') r.id: '${r.values['mimeType']}',
    };
    for (final report in reports) {
      final v = report.values;
      if (report.type == 'candidate-pair' &&
          v['state'] == 'succeeded' &&
          v['currentRoundTripTime'] != null) {
        final out = v['availableOutgoingBitrate'];
        parts.add(
          'round trip ${ms(v['currentRoundTripTime'])}'
          '${out is num ? ', can send ${(out / 1000).round()} kbit/s' : ''}',
        );
      } else if (report.type == 'inbound-rtp' && v['kind'] == 'audio') {
        parts.add(
          'audio in (${codecs[v['codecId']]}): '
          '${v['packetsReceived']} packets, ${v['packetsLost']} lost, '
          'jitter ${ms(v['jitter'])}, '
          'concealed ${v['concealedSamples']} of ${v['totalSamplesReceived']} samples',
        );
      } else if (report.type == 'inbound-rtp' &&
          (v['framesDecoded'] as num? ?? 0) > 0) {
        parts.add(
          'video in (${codecs[v['codecId']]}, ${v['decoderImplementation']}): '
          '${v['framesDecoded']} frames, '
          '${v['frameWidth']}x${v['frameHeight']} at ${v['framesPerSecond']} fps, '
          '${v['packetsLost']} packets lost, ${v['freezeCount']} freezes '
          '(${v['totalFreezesDuration']} s)',
        );
      } else if (report.type == 'outbound-rtp' &&
          '${v['mid']}' == '0' &&
          (v['packetsSent'] as num? ?? 0) > 0) {
        // About 80 bytes a packet in plain Opus, about twice that with RED.
        final sent = v['packetsSent'] as num;
        parts.add(
          'voice out: $sent packets of '
          '${((v['bytesSent'] as num? ?? 0) / sent).toStringAsFixed(1)} bytes',
        );
      } else if (report.type == 'outbound-rtp' &&
          (v['framesEncoded'] as num? ?? 0) > 0) {
        final target = v['targetBitrate'];
        parts.add(
          'video out (${codecs[v['codecId']]}, ${v['encoderImplementation']}): '
          '${v['frameWidth']}x${v['frameHeight']} at '
          '${v['framesPerSecond']} fps'
          '${target is num ? ', ${(target / 1000).round()} kbit/s' : ''}, '
          'limited by ${v['qualityLimitationReason']} '
          '(${v['qualityLimitationResolutionChanges']} resolution changes)',
        );
      }
    }
    _log('quality $when with ${peer.id.substring(0, 8)}: ${parts.join('; ')}');
  }
}

/// Asks for this computer's own devices at once, so a permission prompt shows at the start
/// and not in the middle of the call.
Future<void> _ask() async {
  if (_mic == 'system' || _camera) {
    try {
      final stream = await navigator.mediaDevices.getUserMedia({
        'audio': _mic == 'system',
        'video': _camera,
      });
      for (final track in stream.getTracks()) {
        await track.stop();
      }
      await stream.dispose();
    } catch (e) {
      _log('devices refused: $e');
    }
  }
  if (_share != null) {
    try {
      await desktopCapturer.getSources(types: [SourceType.Screen]);
    } catch (e) {
      _log('screens refused: $e');
    }
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('a call between copies', (tester) async {
    // Never in the app's own data folder: after a permission change, macOS may reopen the
    // app without the test's settings.
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      await _call(tester);
      File('$_dir/$_role-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$_role-result.txt').writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

Future<void> _call(WidgetTester tester) async {
  final others = _names.keys.take(_copies).where((r) => r != _role).toList();
  await app.main();
  await _until(
    tester,
    'welcome',
    () => find.byType(TextField).evaluate().isNotEmpty,
  );
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      _names[_role]!;
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  await _ask();

  if (_role == 'a') {
    // A link takes one request, and a new link replaces the last one: one at a time.
    for (final role in others) {
      final link = await api.friendLink();
      File('$_dir/link-$role.txt').writeAsStringSync(link.text);
      final before = state.contacts.friends.length;
      await _until(
        tester,
        'a friend request',
        () => state.contacts.requests.incoming.isNotEmpty,
        seconds: 900,
      );
      final request = state.contacts.requests.incoming.first;
      await state.answer(request.person.id, true);
      await _until(
        tester,
        'friends with ${request.person.name}',
        () => state.contacts.friends.length > before,
      );
    }
    await state.createGroup('Call', 3, [
      for (final friend in state.contacts.friends) friend.person.id,
    ]);
    await _until(tester, 'group', () => state.contacts.groups.isNotEmpty);
  } else {
    final file = File('$_dir/link-$_role.txt');
    await _until(
      tester,
      'link',
      () => file.existsSync() && file.readAsStringSync().isNotEmpty,
      seconds: 900,
    );
    await state.useInvitation(file.readAsStringSync());
    // Ana adds us to her group: it arrives as an invitation to join.
    await _until(
      tester,
      'invited to the group',
      () => state.contacts.invitations.isNotEmpty,
      seconds: 900,
    );
    await state.answerInvitation(state.contacts.invitations.first.id, true);
    await _until(
      tester,
      'added to the group',
      () => state.contacts.groups.isNotEmpty,
      seconds: 900,
    );
  }

  state.prefs.joinWithMic = _mic == '1' || _mic == 'system' || _mic == 'broken';
  // The noise check: copy a enters as someone who left noise suppression on.
  if (_env['FOGATA_TEST_MIC_CASE'] == 'noise' && _role == 'a') {
    state.prefs.noiseSuppression = true;
  }
  final devices = await navigator.mediaDevices.enumerateDevices();
  _log(
    'devices: ${devices.map((d) => '${d.kind}:${d.label} (${d.deviceId})').join(' | ')}',
  );
  if (_mic == '1' || _mic == 'off') {
    // Only the virtual devices of the test, never the computer's own microphone or speakers.
    // FOGATA_TEST_DEVICES ends their names, when tests beside these have devices of their
    // own.
    final suffix = _env['FOGATA_TEST_DEVICES'] ?? '';
    String pick(String kind, String label) => devices
        .firstWhere(
          (d) => d.kind == kind && d.label.contains('$label$suffix'),
          orElse: () => throw StateError('no $kind called $label'),
        )
        .deviceId;
    _tone = state.prefs.audioInput = pick('audioinput', 'FogataMic');
    state.prefs.audioOutput = pick('audiooutput', 'FogataTest');
    _quiet = pick('audioinput', 'FogataQuiet');
    // Measuring, or judging what each copy plays (echo) or sends (noise): what the
    // others say plays into a silent output, not back into the microphone that every
    // copy shares.
    if (_env['FOGATA_TEST_MEASURE'] != null ||
        _env['FOGATA_TEST_MIC_CASE'] == 'echo' ||
        _env['FOGATA_TEST_MIC_CASE'] == 'noise') {
      state.prefs.audioOutput = pick('audiooutput', 'FogataQuietOut');
    }
    // Were the microphone opened anyway, it would hear only silence.
    if (_mic == 'off') state.prefs.audioInput = _quiet;
  }
  if (_mic == 'system') {
    // This computer's own devices, by a part of their names.
    String? named(String kind, String? name) => name == null || name.isEmpty
        ? null
        : devices
              .firstWhere(
                (d) => d.kind == kind && d.label.contains(name),
                orElse: () => throw StateError('no $kind called $name'),
              )
              .deviceId;
    final input = named('audioinput', _env['FOGATA_TEST_INPUT']);
    final output = named('audiooutput', _env['FOGATA_TEST_OUTPUT']);
    if (input != null) state.prefs.audioInput = input;
    if (output != null) state.prefs.audioOutput = output;
  }
  // A microphone that cannot be opened: only where the copy reaches no sound server at all
  // (tools/call-lab/mic.sh broken), never where it could open the computer's own.
  if (_mic == 'broken' && devices.any((d) => d.kind == 'audioinput')) {
    throw StateError(
      'FOGATA_TEST_MIC=broken needs a copy without sound inputs',
    );
  }
  if (_env['FOGATA_TEST_MIC_CASE'] == 'quit' && _role == 'a') {
    await _quitWhileOpening(tester, state);
    return;
  }
  if (_env['FOGATA_TEST_MIC_CASE'] == 'devices' && _role == 'a') {
    await _listedAroundTest(tester, state, 'before any call');
  }
  // The microphone opening only after the others connected, as on a system slow to open
  // it: what they hear must not depend on which comes first.
  final late = int.tryParse(_env['FOGATA_TEST_MIC_LATE'] ?? '');
  if (late != null) micOpensAfter = Duration(seconds: late);
  // A shared screen's sound in one channel, to compare (mic.sh echo, ECHO_MONO=1).
  if (_env['FOGATA_TEST_SCREEN_SOUND_MONO'] == '1') screenSoundInStereo = false;
  // Copy b enters only once a has been alone in the call a while (see below).
  if (_env['FOGATA_TEST_MIC_CASE'] == 'alone' && _role == 'b') {
    await _until(
      tester,
      'a alone in the call',
      () => File('$_dir/mark-a-alone').existsSync(),
      seconds: 300,
    );
  }
  await state.openRoom(state.contacts.groups.first.id);
  if (const ['broken', 'alsa', 'hang'].contains(_env['FOGATA_TEST_MIC_CASE'])) {
    await _noSoundServer(tester, state, _env['FOGATA_TEST_MIC_CASE']!);
    return;
  }
  await _until(tester, 'in the call', () => state.callState.call != null);
  if (_env['FOGATA_TEST_MIC_CASE'] == 'alone' && _role == 'a') {
    // Alone in the call a while, as when the others join later: the call reads this
    // device's own level through a loop of its own meanwhile (AloneLevel). Copy b enters
    // only once this is marked.
    await _wait(tester, const Duration(seconds: 6));
    if (state.callState.call!.peers.values.any((p) => p.connected)) {
      throw StateError('not alone in the call');
    }
    _mark('alone');
  }
  await _until(
    tester,
    'connected to everyone',
    () =>
        state.callState.call!.peers.length >= others.length &&
        state.callState.call!.peers.values.every((p) => p.connected),
    seconds: 900,
  );
  await _quality(state, 'at the start');
  if (_env['FOGATA_TEST_QUALITY'] != null) {
    await _connectionQuality(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_TYPING'] != null) {
    await _typing(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_GROUP'] != null) {
    final group = state.roomState.roomGroup!;
    await _groupCall(tester, state);
    await state.openRoom(group);
    await _until(
      tester,
      'back in the group',
      () => (state.roomState.room?.participants.length ?? 0) == _copies,
      seconds: 120,
    );
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  final noise = int.tryParse(_env['FOGATA_TEST_NOISE'] ?? '');
  if (noise != null) {
    await _noiseSteps(tester, state, noise);
    await _say(state, 'done');
    await _hear(tester, state, ['done'], seconds: 4 * noise + 180);
    return;
  }
  final start = int.tryParse(_env['FOGATA_TEST_CAMERA_START'] ?? '');
  if (start != null) {
    await _cameraStart(tester, state, start);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  final steps = int.tryParse(_env['FOGATA_TEST_CAMERA_STEPS'] ?? '');
  if (steps != null) {
    await _cameraSteps(tester, state, steps);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_HOSTILE'] == 'offers') {
    await _hostileOffers(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_HOSTILE'] == 'unanswered') {
    await _hostileUnanswered(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_HOSTILE'] == 'stuck') {
    await _hostileStuck(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_HOSTILE'] == 'giant') {
    await _hostileGiant(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  if (_env['FOGATA_TEST_HOSTILE'] != null) {
    await _hostile(tester, state);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }
  final measure = int.tryParse(_env['FOGATA_TEST_MEASURE'] ?? '');
  if (measure != null) {
    await _measureCall(tester, state, measure);
    await _say(state, 'done');
    await _hear(tester, state, ['done'], seconds: 300);
    return;
  }
  final micCase = _env['FOGATA_TEST_MIC_CASE'];
  if (micCase != null) {
    await _micCase(tester, state, micCase);
    await _say(state, 'done');
    // Measuring many tests or calls (see [_soakTests]) takes a while longer.
    final soaking =
        _env['FOGATA_TEST_MIC_SOAK'] != null ||
        _env['FOGATA_TEST_CALL_SOAK'] != null;
    await _hear(tester, state, ['done'], seconds: soaking ? 1800 : 300);
    return;
  }
  final watch = int.tryParse(_env['FOGATA_TEST_WATCH'] ?? '');
  if (watch != null) {
    await _watchScreen(tester, state, watch);
    await _say(state, 'done');
    await _hear(tester, state, ['done']);
    return;
  }

  await _voice(tester, state);
  await _cameras(tester, state);
  await _screens(tester, state);
  await _screenQuality(tester, state);

  await _quality(state, 'at the end');
  await _say(state, 'done');
  await _hear(tester, state, ['done']);
  if (_copies == 2) {
    await _groups(tester, state);
    await _video(tester, state);
    await _changesInRoom(tester, state);
    await _direct(tester, state);
    await _directMore(tester, state);
    await _card(tester, state);
  }
}

/// Who shows as typing under the room's chat. Copy a watches; the others start writing one
/// after the other (b, then c, then d), changing their text every second without sending,
/// each once a has seen the one before: a shows "Bea is typing…", then two names, then
/// "Several people are typing…" (with four copies or more). Then b sends her message, c
/// clears her box and d just stops: b and c go from a's line at once, d only when her last
/// signal runs out. The copies tell each other the steps with files in FOGATA_CALL, since a
/// message in the chat would end the writer's typing.
Future<void> _typing(WidgetTester tester, AppState state) async {
  final writers = _names.keys.take(_copies).skip(1).take(3).toList();
  File step(String name) => File('$_dir/typing-$name');
  final box = find.byWidgetPredicate(
    (w) => w is TextField && w.decoration?.hintText == 'Message Call',
  );
  String line() {
    final found = find.byKey(const ValueKey('typing-line'));
    if (found.evaluate().isEmpty) return '';
    return tester.widget<Text>(found.first).textSpan!.toPlainText();
  }

  if (_role == 'a') {
    final expected = [
      'Bea is typing…',
      'Bea and Carla are typing…',
      'Several people are typing…',
    ];
    for (var i = 0; i < writers.length; i++) {
      step('go-$i').writeAsStringSync('go');
      await _until(
        tester,
        'shows: ${expected[i]}',
        () => line() == expected[i],
        seconds: 60,
      );
      await _shot(tester, 'typing-a-${i + 1}');
    }
    final ended = DateTime.now();
    step('end').writeAsStringSync('end');
    // When each one no longer shows, in ms after the end.
    final gone = <String, int>{};
    void note() {
      final now = DateTime.now().difference(ended).inMilliseconds;
      final shown = line();
      for (final role in writers) {
        if (!shown.startsWith('Several') && !shown.contains(_names[role]!)) {
          gone.putIfAbsent(role, () => now);
        }
      }
    }

    final quick = writers.take(2).toList();
    await _until(tester, 'Bea and Carla gone', () {
      note();
      return quick.every(gone.containsKey);
    }, seconds: 30);
    if (writers.length >= 3) {
      if (line() != 'Dani is typing…') {
        throw StateError('Dani should still show: “${line()}”');
      }
      await _shot(tester, 'typing-a-4');
    }
    await _until(tester, 'nobody typing', () {
      note();
      return line().isEmpty;
    }, seconds: 30);
    _log('typing: gone after (ms) $gone');
    await _shot(tester, 'typing-a-5');
    step('checked').writeAsStringSync('checked');
    for (final role in quick) {
      if (gone[role]! > 3000) {
        throw StateError(
          '${_names[role]} showed ${gone[role]} ms after ending',
        );
      }
    }
    if (writers.length >= 3 && gone[writers[2]]! < 2500) {
      throw StateError('Dani went before her time: ${gone[writers[2]]} ms');
    }
    return;
  }
  final mine = writers.indexOf(_role);
  if (mine < 0) return;
  await _until(
    tester,
    'my turn to write',
    () => step('go-$mine').existsSync(),
    // Until a is connected to everyone too, which may take a while.
    seconds: 1200,
  );
  await _until(tester, 'the message box', () => box.evaluate().isNotEmpty);
  final controller = tester.widget<TextField>(box.first).controller!;
  var n = 0;
  final seen = <String>{};
  while (!step('end').existsSync()) {
    controller.text = 'typing ${n++}';
    await _wait(tester, const Duration(seconds: 1));
    // Never this copy's own name.
    if (line().contains(_names[_role]!)) {
      throw StateError('shows its own typing: ${line()}');
    }
    seen.add(line());
  }
  _log('typing: saw $seen');
  if (_role == 'b') {
    await _tap(tester, find.byTooltip('Send'));
  } else if (_role == 'c') {
    controller.clear();
  }
  await _shot(tester, 'typing-$_role-end');
  // Until a has seen the last one go: a message in the chat would end it at once.
  await _until(
    tester,
    'a checked the end',
    () => step('checked').existsSync(),
    seconds: 120,
  );
}

/// How each person's connection shows while the network of one copy gets worse step by
/// step: FOGATA_QUALITY_SLOW names it (b when left out). The copies cannot change the
/// network themselves; whatever runs them can, with network namespaces and netem for
/// example. Before each step it writes `impair-<step>` to FOGATA_CALL, and each copy
/// writes `quality-<role>-<step>` there once every connection shows what the step gives:
/// a slower path (okay), losses (weak), nothing at all (lost) and the network as before
/// (back). Only the connections of the slow copy change; with three copies or more, the
/// slow one also tells the others that its own network is the poor one.
Future<void> _connectionQuality(WidgetTester tester, AppState state) async {
  final slow = _names[_env['FOGATA_QUALITY_SLOW'] ?? 'b']!;
  final call = state.callState.call!;
  final quality = call.quality;
  final ids = call.peers.keys.toList();
  String nameOf(String id) => state.roomState.room!.participants
      .firstWhere((p) => p.person.id == id)
      .person
      .name;
  final mine = _names[_role] == slow;
  final slowId = mine ? null : ids.firstWhere((id) => nameOf(id) == slow);
  // What the connection to [id] shows in a step that gives [level] to the slow copy.
  NetLevel expected(String id, NetLevel level) =>
      mine || id == slowId ? level : NetLevel.good;
  Finder pill(String id) => find.descendant(
    of: find.byWidgetPredicate((w) => w is TileQuality && w.id == id),
    matching: find.byType(Pressable),
  );
  Future<void> details(Finder bars, String name) async {
    await _tap(tester, bars);
    await _wait(tester, const Duration(milliseconds: 700));
    await _shot(tester, name);
    await _escape(tester);
    await _wait(tester, const Duration(milliseconds: 300));
  }

  for (final (step, level) in [
    ('good', NetLevel.good),
    ('okay', NetLevel.okay),
    ('weak', NetLevel.weak),
    ('lost', NetLevel.lost),
    ('back', NetLevel.good),
  ]) {
    if (step != 'good') {
      await _until(
        tester,
        'network made $step',
        () => File('$_dir/impair-$step').existsSync(),
        seconds: 600,
      );
    }
    final since = DateTime.now();
    await _until(
      tester,
      'connections $step',
      () => ids.every((id) => quality.linkOf(id) == expected(id, level)),
      seconds: step == 'back' ? 180 : 90,
    );
    final took = DateTime.now().difference(since);
    if (ids.length >= 2 && level != NetLevel.lost) {
      // The slow copy tells the others its own network is the one at fault.
      await _until(
        tester,
        'own networks $step',
        () =>
            quality.network == (mine ? level : NetLevel.good) &&
            (slowId == null || quality.theirsOf(slowId) == level),
        seconds: 30,
      );
    }
    _log(
      'quality $step after ${took.inMilliseconds} ms: '
      '${ids.map((id) => '${nameOf(id)} ${quality.linkOf(id)?.word}, '
          'own ${quality.theirsOf(id)?.word ?? '-'}, ${quality.delayOf(id)} ms, '
          '${quality.lossOf(id)?.toStringAsFixed(1) ?? '-'} % lost').join('; ')}; '
      'network here ${quality.network?.word ?? '-'}, shown ${quality.mine?.word}',
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'quality-$_role-$step');
    if (step == 'weak' || step == 'lost') {
      if (slowId != null) {
        await details(pill(slowId), 'quality-$_role-$step-details');
      } else {
        await details(
          find.descendant(
            of: find.byType(OwnQuality),
            matching: find.byType(Pressable),
          ),
          'quality-$_role-$step-own',
        );
      }
    }
    File('$_dir/quality-$_role-$step')
        .writeAsStringSync('${took.inMilliseconds}\n');
  }
}

/// Two copies, after the direct calls: Ana changes her banner and Bea opens Ana's card,
/// which shows it. Then Bea removes Ana from her friends, and Ana loses Bea too.
Future<void> _card(WidgetTester tester, AppState state) async {
  const green = 0xFF0E7C66;
  final friend = state.contacts.friends.single.person;
  state.go(Screen.friends);
  if (_role == 'a') {
    await state.setBannerColor(const Color(green));
  } else {
    await _tap(tester, find.byKey(ValueKey('friend-row:${friend.id}')));
    await _until(
      tester,
      "${friend.name}'s card with her banner",
      () => state.contacts.friendBanners[friend.id]?.color == green,
      seconds: 60,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'card-b-friend');
    state.closeFriendCard();
    await state.removeFriend(friend.id);
  }
  await _until(
    tester,
    'no longer friends with ${friend.name}',
    () => state.contacts.friends.isEmpty,
    seconds: 120,
  );
  await _wait(tester, const Duration(milliseconds: 600));
  await _shot(tester, 'card-$_role-removed');
  _log('the card shown and the friend removed on both sides');
}

/// Two copies, in the room after the video: Ana gives the group a picture and herself
/// another avatar, which both see at once, her own tile too. Then Bea leaves the group
/// while in its room: the call ends without bringing the application down.
Future<void> _changesInRoom(WidgetTester tester, AppState state) async {
  final group = state.roomState.roomGroup!;
  const words = [
    'picture 0',
    'picture 1',
    'picture 2',
    'picture 3',
    'picture 4',
  ];
  UiAvatar? avatarOf(String id) => state.roomState.room?.participants
      .where((p) => p.person.id == id)
      .firstOrNull
      ?.person
      .avatar;
  if (_role == 'a') {
    final me = state.contacts.profile!.person;
    final preset = (me.avatar.preset + 1) % 5;
    await state.setGroupPicture(group, await _groupPicture());
    await state.updateProfile(me.name, preset, null);
    await _until(
      tester,
      'my own new picture in the room',
      () => avatarOf(me.id)?.preset == preset,
    );
    await _say(state, 'picture $preset');
    await _until(
      tester,
      'the group picture in the title bar',
      () => state.contacts.group(group)?.picture != null,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'changes-a-picture');
    // Home while in the room: its card offers to go back to it.
    state.go(Screen.home);
    await _until(
      tester,
      'the way back to the room on Home',
      () => find.text('Return to room').evaluate().isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'changes-a-home');
    state.go(Screen.room);
    await _until(
      tester,
      'Bea gone from the group',
      () => state.contacts.group(group)?.members == 1,
      seconds: 120,
    );
  } else {
    final said = await _hear(tester, state, words);
    final ana = said.keys.single;
    final preset = int.parse(said[ana]!.split(' ').last);
    await _until(
      tester,
      "${ana.name}'s new picture in the room",
      () => avatarOf(ana.id)?.preset == preset,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'changes-b-picture');
    await state.leaveRoom();
    await state.succeeds(() => api.leaveGroup(groupId: group));
    await state.refreshGroups();
    await _until(
      tester,
      'out of the group',
      () => state.contacts.group(group) == null,
    );
  }
  _log('pictures changed in the room, and the group left during the call');
}

/// A small picture for the group: two circles on a purple square.
Future<Uint8List> _groupPicture() async {
  final recorder = ui.PictureRecorder();
  final canvas = Canvas(recorder);
  canvas.drawRect(
    const Rect.fromLTWH(0, 0, 64, 64),
    Paint()..color = const Color(0xFF5A1E9E),
  );
  canvas.drawCircle(
    const Offset(20, 20),
    18,
    Paint()..color = const Color(0xFFE0245E),
  );
  canvas.drawCircle(
    const Offset(48, 46),
    16,
    Paint()..color = const Color(0xFF2563EB),
  );
  final image = await recorder.endRecording().toImage(64, 64);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  return data!.buffer.asUint8List();
}

/// Two copies: Ana shares the test clip (FOGATA_CLIP) and Bea sees its first picture and
/// length in the chat, then plays it, deafened so it makes no sound.
Future<void> _video(WidgetTester tester, AppState state) async {
  const name = 'clip.mp4';
  if (_role == 'a') {
    final clip = _env['FOGATA_CLIP'];
    if (clip == null) throw StateError('FOGATA_CLIP is not set');
    await state.addOutgoing([clip]);
    await state.sendOutgoing('');
  } else {
    await _until(
      tester,
      'the video with its first picture',
      () => (state.roomState.room?.files ?? const <UiFile>[]).any(
        (f) => f.name == name && f.video != null,
      ),
      seconds: 120,
    );
    final file = state.roomState.room!.files.firstWhere((f) => f.name == name);
    final video = file.video!;
    if (video.width != 480 || video.height != 270) {
      throw StateError('the video is ${video.width}x${video.height}');
    }
    if ((video.durationMs - 4000).abs() > 150) {
      throw StateError('the video lasts ${video.durationMs} ms');
    }
    await _wait(tester, const Duration(seconds: 1));
    await _shot(tester, 'video-b-card');
    await state.setDeafened(true);
    // The chat may still be moving: the card is pressed by its own handler.
    final card = find.byKey(ValueKey('video:${file.id}'));
    await _until(tester, 'the video card', () => card.evaluate().isNotEmpty);
    _log('opening the video');
    tester.widget<Pressable>(card).onTap!();
    await _wait(tester, const Duration(seconds: 1));
    _log('the video opened');
    await _shot(tester, 'video-b-opened');
    await _until(
      tester,
      'the video playing',
      () => find.textContaining('0:02 / 0:04').evaluate().isNotEmpty,
      seconds: 120,
    );
    await _shot(tester, 'video-b-playing');
    await _escape(tester);
    await _wait(tester, const Duration(milliseconds: 600));
    await state.setDeafened(false);
    await _say(state, 'video played');
  }
  if (_role == 'a') await _hear(tester, state, ['video played']);
  _log('a video shared and played');
}

/// Two copies: Ana opens a second voice room and Bea moves into it, each room on its own;
/// Ana's group settings; then Ana makes a group of her own and says its invite link in the
/// chat, Bea asks to join with it and Ana lets her in from the bell.
Future<void> _groups(WidgetTester tester, AppState state) async {
  final group = state.contacts.groups.firstWhere((g) => g.name == 'Call').id;
  if (_role == 'a') await state.setVoiceRoom(group, null, 'Gaming');
  await _until(
    tester,
    'two voice rooms',
    () => (state.contacts.group(group)?.voiceRooms.length ?? 0) == 2,
    seconds: 60,
  );
  final gaming = state.contacts.group(group)!.voiceRooms[1].id;
  int inRoom(int i) =>
      state.contacts.group(group)?.voiceRooms[i].room?.participants.length ?? 0;
  if (_role == 'b') await state.openRoom(group, voice: gaming);
  await _until(
    tester,
    'Bea in Gaming and Ana in General',
    () => inRoom(0) == 1 && inRoom(1) == 1,
    seconds: 60,
  );
  await _wait(tester, const Duration(seconds: 1));
  await _shot(tester, 'groups-$_role-voice-rooms');
  // What is said in one voice room stays there.
  await state.sendChat('only in ${_role == 'a' ? 'General' : 'Gaming'}');
  await _wait(tester, const Duration(seconds: 3));
  final heard = [
    for (final line in state.roomState.room?.chat ?? const <UiChatLine>[])
      if (!line.mine && line.text.startsWith('only in')) line.text,
  ];
  if (heard.isNotEmpty) {
    throw StateError('one voice room heard the other: $heard');
  }
  if (_role == 'b') await state.openRoom(group, voice: AppState.firstRoom);
  await _until(
    tester,
    'both back in General',
    () => inRoom(0) == 2,
    seconds: 60,
  );
  _log('voice rooms apart');

  // Ana lowers the most people per room to 2: both see General full as it arrives, in the
  // column and the header, and Ana puts it back.
  if (_role == 'a') {
    await state.setGroupLimits(
      group,
      state.contacts.group(group)!.maxMembers,
      2,
    );
  }
  await _until(
    tester,
    'the new limit',
    () => state.contacts.group(group)?.maxInRoom == 2,
    seconds: 60,
  );
  await _until(
    tester,
    'General shown full',
    () =>
        find.text('2/2', findRichText: true).evaluate().length == 2 &&
        find.text('Full · ').evaluate().isNotEmpty &&
        find.text('0/2', findRichText: true).evaluate().isNotEmpty,
    seconds: 10,
  );
  await _shot(tester, 'groups-$_role-full');
  await _say(state, 'saw it full');
  await _hear(tester, state, ['saw it full']);
  if (_role == 'a') {
    await state.setGroupLimits(
      group,
      state.contacts.group(group)!.maxMembers,
      8,
    );
  }
  await _until(
    tester,
    'the limit back',
    () =>
        state.contacts.group(group)?.maxInRoom == 8 &&
        find.text('2/8', findRichText: true).evaluate().length == 2,
    seconds: 60,
  );
  _log('limits shown as they change');

  // The chat beside the room is hidden by hand and shown again; on a wide window the
  // choice is kept in the settings. A small window starts with it hidden.
  final small = state.roomState.roomSmall;
  if (small) {
    await _tap(tester, find.byTooltip('Show the chat'));
    await _wait(tester, const Duration(milliseconds: 700));
  }
  await _tap(tester, find.byTooltip('Hide the chat'));
  await _wait(tester, const Duration(milliseconds: 700));
  final prefs = File('${_env['FOGATA_DATA_DIR']}/app.json');
  if (state.roomChatShown ||
      (!small && !prefs.readAsStringSync().contains('"roomChat":false')) ||
      find.byTooltip('Show the chat').evaluate().isEmpty) {
    throw StateError('the chat did not hide');
  }
  await _shot(tester, 'groups-$_role-chat-hidden');
  await _tap(tester, find.byTooltip('Show the chat'));
  await _wait(tester, const Duration(milliseconds: 700));
  if (!state.roomChatShown ||
      !prefs.readAsStringSync().contains('"roomChat":true')) {
    throw StateError('the chat did not come back');
  }
  _log('chat hidden and shown');

  if (_role == 'a') {
    for (final (tab, name) in const [
      (0, 'general'),
      (1, 'rooms'),
      (2, 'members'),
      (3, 'invite'),
      (4, 'limits'),
    ]) {
      state.openGroupSettings(group, tab: tab);
      await _wait(tester, const Duration(milliseconds: 700));
      await _shot(tester, 'groups-a-settings-$name');
    }
    state.closeGroupSettings();
    await _wait(tester, const Duration(milliseconds: 500));

    final own = await state.createGroup('By link', 4, []);
    // A link carries addresses and is longer than a chat message: it goes in pieces.
    final invite = await api.invite(groupId: own!, days: 1);
    const piece = 1500;
    final pieces = (invite.length + piece - 1) ~/ piece;
    for (var i = 0; i < pieces; i++) {
      final end = (i + 1) * piece < invite.length
          ? (i + 1) * piece
          : invite.length;
      await state.sendChat(
        'test: invite ${i + 1}/$pieces ${invite.substring(i * piece, end)}',
      );
    }
    await _until(
      tester,
      'Bea asking to join',
      () => state.contacts.joinAsks.isNotEmpty,
      seconds: 180,
    );
    await _tap(tester, find.byTooltip('Notifications'));
    await _wait(tester, const Duration(milliseconds: 700));
    await _shot(tester, 'groups-a-bell');
    await _tap(tester, find.text('Let in'));
    await _hear(tester, state, ['joined by link']);
    // The bell's list stays open until a click beside it.
    await tester.tapAt(const Offset(30, 400));
    await _until(
      tester,
      'the bell closed',
      () => state.notices.bellFrom == null,
    );
  } else {
    String? invite;
    await _until(tester, 'the invite link', () {
      final pieces = <int, String>{};
      var count = 0;
      for (final line in state.roomState.room?.chat ?? const <UiChatLine>[]) {
        final m = RegExp(r'^test: invite (\d+)/(\d+) (.*)$')
            .firstMatch(line.text);
        if (line.mine || m == null) continue;
        pieces[int.parse(m[1]!)] = m[3]!;
        count = int.parse(m[2]!);
      }
      if (count == 0 || pieces.length < count) return false;
      invite = [for (var i = 1; i <= count; i++) pieces[i]].join();
      return true;
    }, seconds: 180);
    await state.useInvitation(invite!);
    await _until(
      tester,
      'let in by Ana',
      () => state.contacts.groups.any((g) => g.name == 'By link'),
      seconds: 180,
    );
    await _say(state, 'joined by link');
  }
  _log('joined with a link someone in the group let in');
}

/// As long as a name may be.
const _longName = 'Ana Isabel Montenegro de la Vega Ramírez';

/// Two copies: Ana calls Bea directly, Bea answers from the card, both hear the way in,
/// they write in the call's chat, and Ana hangs up. Then Bea declines a call and misses
/// another from Ana under a long name, which the note shows whole.
Future<void> _direct(WidgetTester tester, AppState state) async {
  await state.leaveRoom();
  await _until(tester, 'out of the room', () => state.callState.call == null);
  final friend = state.contacts.friends.single.person;
  final soundsBefore = state.played.length;
  if (_role == 'a') {
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(friend.id);
    await _until(
      tester,
      'ringing ${friend.name}',
      () => find.textContaining('Calling ${friend.name}').evaluate().isNotEmpty,
    );
    await _shot(tester, 'direct-a-calling');
  } else {
    // Bea waits in Friends, where hanging up brings her back.
    state.go(Screen.friends);
    await _until(
      tester,
      'rung by ${friend.name}',
      () => state.callState.incomingCall != null,
      seconds: 120,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-b-incoming');
    await _tap(tester, find.text('Accept'));
  }
  await _until(
    tester,
    'in the call with ${friend.name}',
    () =>
        state.callState.activeCall?.phase == UiCallPhase.connected &&
        (state.callState.call?.peers[friend.id]?.connected ?? false),
    seconds: 120,
  );
  _log('direct call connected');
  await _until(
    tester,
    'the sound of coming into the call',
    () => state.played.skip(soundsBefore).contains(UiSound.join),
    seconds: 10,
  );
  // The call's group is not among the groups.
  final callGroup = state.callState.activeCall!.groupId;
  if (state.contacts.group(callGroup) != null) {
    throw StateError('the call shows as a group');
  }
  await _say(state, 'in direct call');
  await _until(
    tester,
    '${friend.name} in the call chat',
    () => _said(state, ['in direct call']).isNotEmpty,
    seconds: 60,
  );
  await _wait(tester, const Duration(seconds: 2));
  await _shot(tester, 'direct-$_role-in-call');
  if (_role == 'a') {
    // Elsewhere in the window, the call shows in the corner; a click goes back to it.
    state.goHome();
    final card = find.byKey(const ValueKey('in-call-card'));
    await _until(
      tester,
      'the call in the corner',
      () => card.evaluate().isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-a-elsewhere');
    await _tap(tester, card);
    await _until(
      tester,
      'back in the call',
      () => state.nav.page == Screen.room,
    );
    await _wait(tester, const Duration(seconds: 1));
    await _tap(tester, find.text('Hang up'));
  } else {
    // Bea stays, waiting a minute for Ana to come back, and hangs up herself.
    await _until(
      tester,
      'waiting for ${friend.name} to come back',
      () => state.callState.activeCall?.aloneSinceMs != null,
      seconds: 60,
    );
    await _wait(tester, const Duration(seconds: 1));
    await _shot(tester, 'direct-b-waiting');
    // Ana sees her way back to Bea first; she says so in their chat, which reaches a copy
    // on another computer too.
    await _until(
      tester,
      '${friend.name} saw the way back',
      () => state.direct.chat(friend.id)?.last == 'test: saw the way back',
      seconds: 45,
    );
    await _tap(tester, find.text('Hang up'));
  }
  final back = _role == 'a' ? Screen.home : Screen.friends;
  await _until(
    tester,
    'the call over, back in ${back.name}',
    () => state.callState.activeCall == null && state.nav.page == back,
    seconds: 60,
  );
  _log('direct call ended');
  if (_role == 'a') {
    // Bea is still in the call Ana left: her Call buttons take Ana back into it.
    await _until(
      tester,
      '${friend.name} still in the call',
      () => state.callState.callReturns.contains(friend.id),
      seconds: 20,
    );
    state.direct.show(friend.id);
    final call = find.byKey(const ValueKey('direct-call'));
    await _until(
      tester,
      'Return to call in the chat',
      () =>
          call.evaluate().isNotEmpty &&
          tester.widget<FgButton>(call).text == 'Return to call',
    );
    await _shot(tester, 'direct-a-return-chat');
    state.go(Screen.friends);
    await _until(
      tester,
      'Return to call in Friends',
      () =>
          find.widgetWithText(FgButton, 'Return to call').evaluate().isNotEmpty,
    );
    await _shot(tester, 'direct-a-return-friends');
    await state.direct.show(friend.id);
    await state.direct.send('test: saw the way back');
    // Bea hangs up too: the call is over, and Call is back.
    await _until(
      tester,
      'Call again for ${friend.name}',
      () =>
          !state.callState.callReturns.contains(friend.id) &&
          find.widgetWithText(FgButton, 'Return to call').evaluate().isEmpty,
      seconds: 30,
    );
  }
  // The chat says the call took place, on both sides.
  state.direct.show(friend.id);
  final line = find.byWidgetPredicate(
    (w) =>
        w.key is ValueKey<String> &&
        (w.key! as ValueKey<String>).value.startsWith('direct-call-line:'),
  );
  await _until(
    tester,
    'the call in the chat',
    () => line.evaluate().isNotEmpty,
    seconds: 30,
  );
  final said = find.textContaining('started a call that lasted');
  _log('call line: ${tester.widget<Text>(said.first).data}');
  final starter = _role == 'a' ? 'You' : friend.name;
  if (find.textContaining('$starter started a call').evaluate().isEmpty) {
    throw StateError('the call line does not say $starter started it');
  }
  await _shot(tester, 'direct-$_role-call-line');
  state.goHome();

  // Bea declines the next call; then Ana rings and stops before Bea answers.
  if (_role == 'a') {
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(friend.id);
    await _until(
      tester,
      'declined',
      () => find.textContaining('can’t talk right now').evaluate().isNotEmpty,
      seconds: 60,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-a-declined');
    await _tap(tester, find.text('Close'));
    final me = state.contacts.profile!.person;
    await state.updateProfile(_longName, me.avatar.preset, me.avatar.photo);
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(friend.id);
    await _wait(tester, const Duration(seconds: 3));
    await _tap(tester, find.text('Cancel'));
    await _until(
      tester,
      'stopped ringing',
      () => state.callState.activeCall == null && state.nav.page == Screen.home,
    );
  } else {
    await _until(
      tester,
      'rung again',
      () => state.callState.incomingCall != null,
      seconds: 60,
    );
    await _tap(tester, find.text('Decline'));
    await _until(
      tester,
      'rung a third time',
      () => state.callState.incomingCall != null,
      seconds: 60,
    );
    // A click beside the card puts the ring in the corner, where it goes on.
    await tester.tapAt(const Offset(30, 400));
    await _until(
      tester,
      'the ring aside',
      () => state.callState.ringAside != null,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-b-aside');
    await _until(
      tester,
      'a missed call',
      () => state.notices.missedNote != null,
      seconds: 60,
    );
    await _until(
      tester,
      'the missed call under the long name',
      () => find.textContaining(_longName).evaluate().isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    final note = tester.renderObject<RenderParagraph>(
      find.textContaining('Missed call from'),
    );
    if (note.didExceedMaxLines) {
      throw StateError('the missed call note cut the name');
    }
    await _shot(tester, 'direct-b-missed');
    // Like a toast, the note goes by itself; the missed call stays in the bell.
    await _until(
      tester,
      'the missed call note gone by itself',
      () =>
          state.notices.missedNote == null &&
          find.textContaining('Missed call from').evaluate().isEmpty,
      seconds: Notices.missedNoteFor.inSeconds + 3,
    );
    if (!state.notices.list.any((n) => n.kind == NoticeKind.missedCall)) {
      throw StateError('the missed call left the bell too');
    }
    await _tap(tester, find.widgetWithText(Pressable, 'Friends').first);
    await _until(
      tester,
      'the missed call in Friends',
      () => find.textContaining('Missed call').evaluate().isNotEmpty,
    );
    await _shot(tester, 'direct-b-missed-friends');
  }
  _log('declined and missed calls shown');
}

/// Two copies, after [_direct]: Ana answers Bea's call from the settings, which stay open
/// with what she was writing; then Ana waits alone in the group's room and answers Bea from
/// there, asked first whether to close the room; then Bea, rung by Ana, puts the ring aside
/// and presses Call on Ana, which answers Ana's call.
Future<void> _directMore(WidgetTester tester, AppState state) async {
  final friend = state.contacts.friends.single.person;
  Finder field(String text) => find.byWidgetPredicate(
    (w) => w is TextField && w.controller?.text == text,
  );
  Future<void> connected(String what) => _until(
    tester,
    what,
    () =>
        state.callState.activeCall?.phase == UiCallPhase.connected &&
        (state.callState.call?.peers[friend.id]?.connected ?? false),
    seconds: 120,
  );
  // The one left waiting for the other to come back hangs up too.
  Future<void> over(String what) async {
    await _until(
      tester,
      what,
      () =>
          (state.callState.activeCall == null &&
              state.callState.call == null) ||
          state.callState.activeCall?.aloneSinceMs != null,
      seconds: 60,
    );
    if (state.callState.activeCall == null) return;
    if (!state.inCallPage) state.showCall();
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Hang up'));
    await _until(
      tester,
      '$what, here too',
      () => state.callState.activeCall == null && state.callState.call == null,
      seconds: 30,
    );
  }

  // A call answered from the settings keeps them open, with what was being written.
  if (_role == 'a') {
    state.goHome();
    state.openSettings();
    final me = state.contacts.profile!.person.name;
    await _until(tester, 'the settings', () => field(me).evaluate().isNotEmpty);
    tester.widget<TextField>(field(me).first).controller!.text = 'Ana Writing';
    await _wait(tester, const Duration(milliseconds: 300));
    await _until(
      tester,
      'rung in the settings',
      () => state.callState.incomingCall != null,
      seconds: 120,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Accept'));
    await connected('in the call from the settings');
    await _wait(tester, const Duration(seconds: 1));
    if (!state.nav.settingsOpen) {
      throw StateError('answering closed the settings');
    }
    if (field('Ana Writing').evaluate().isEmpty) {
      throw StateError('answering lost what was written in the settings');
    }
    await _shot(tester, 'direct-a-settings-kept');
    // The name written there was not saved: closing asks, and it is let go.
    state.closeSettings();
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Discard'));
    await _wait(tester, const Duration(seconds: 1));
    if (state.nav.settingsOpen) throw StateError('the settings did not close');
    if (!state.inCallPage) {
      throw StateError('the call is not behind the settings');
    }
    await _tap(tester, find.text('Hang up'));
  } else {
    state.goHome();
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(friend.id);
    await connected('in the call with ${friend.name}, in her settings');
  }
  await over('the call from the settings over');

  // Answering in a room this device is alone in asks first, as leaving it does.
  if (_role == 'a') {
    // A group of both, its room empty.
    final group = state.contacts.groups
        .firstWhere(
          (g) => g.online > 0 && (g.room?.participants.isEmpty ?? true),
        )
        .id;
    await state.openRoom(group);
    await _until(
      tester,
      'alone in the room',
      () => state.callState.call != null,
    );
    await _until(
      tester,
      'rung in the room',
      () => find.text('Leave and answer').evaluate().isNotEmpty,
      seconds: 120,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Leave and answer'));
    await _until(
      tester,
      'asked whether to close the room',
      () => find.text('Leave and close the room?').evaluate().isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-a-answer-closes-room');
    await _tap(tester, find.text('Leave and close'));
    await connected('in the call, out of the room');
    if (state.contacts.joinedGroup != null) {
      throw StateError('still in the room');
    }
  } else {
    await _until(
      tester,
      '${friend.name} in a group’s room',
      () =>
          state.contacts.group(state.contacts.friends.single.roomGroup) != null,
      seconds: 60,
    );
    await _wait(tester, const Duration(seconds: 1));
    await state.callFriend(friend.id);
    await connected('in the call with ${friend.name}, from her room');
    await _wait(tester, const Duration(seconds: 2));
    state.showCall();
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Hang up'));
  }
  await over('the call from the room over');

  // Calling back a friend whose ring waits answers that ring.
  if (_role == 'a') {
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(friend.id);
    await connected('answered by ${friend.name} calling back');
    await _wait(tester, const Duration(seconds: 2));
    if (!state.inCallPage) state.showCall();
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Hang up'));
  } else {
    state.go(Screen.friends);
    await _until(
      tester,
      'rung by ${friend.name} once more',
      () => state.callState.incomingCall != null,
      seconds: 60,
    );
    final rung = state.callState.incomingCall!.groupId;
    await _wait(tester, const Duration(milliseconds: 600));
    await tester.tapAt(const Offset(30, 400));
    await _until(
      tester,
      'the ring aside',
      () => state.callState.ringAside == rung,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'direct-b-call-back-aside');
    final call = find.descendant(
      of: find.byKey(ValueKey('friend-row:${friend.id}')),
      matching: find.byWidgetPredicate(
        (w) => w is FgButton && (w.text == 'Call' || w.text == 'Call back'),
      ),
    );
    await _tap(tester, call);
    await connected('in ${friend.name}’s call by calling back');
    if (state.callState.activeCall!.groupId != rung) {
      throw StateError('calling back rang instead of answering');
    }
    if (state.callState.calls.length != 1) {
      throw StateError('two calls at once');
    }
  }
  await over('the last direct call over');
  if (state.notices.missedCalls.isNotEmpty && _role == 'a') {
    throw StateError('a call answered shows as missed');
  }
  _log('direct calls from the settings, a room and a call back');
}

/// FOGATA_TEST_GROUP=1, three or four copies, Ana friends with all and the others only
/// with Ana: Ana calls Bea, rings Carla into the call from the call's own button (and Dani
/// with four), and everyone sees everyone, by name, friends or not. Bea hangs up: the
/// others go on and see that she may come back; she calls Ana and is back in the same
/// call. Then all but Carla hang up: Carla is left waiting, and after a minute the call
/// ends for her.
Future<void> _groupCall(WidgetTester tester, AppState state) async {
  // Nobody leaves the group's room before everyone is connected to everyone there: a copy
  // that left first would keep the last one from ever seeing that.
  await _say(state, 'to the group call');
  await _hear(tester, state, ['to the group call']);
  await state.leaveRoom();
  await _until(tester, 'out of the room', () => state.callState.call == null);
  final byName = {
    for (final f in state.contacts.friends) f.person.name: f.person,
  };
  UiPerson named(String role) => byName[_names[role]!]!;
  final everyone = _copies;
  int inCall() => state.roomState.room?.participants.length ?? 0;
  Future<void> all(int n, String what) => _until(
    tester,
    what,
    () => state.callState.call != null && inCall() == n,
    seconds: 120,
  );
  Future<void> answer() async {
    await _until(
      tester,
      'rung',
      () => state.callState.incomingCall != null,
      seconds: 180,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'group-$_role-incoming');
    await _tap(tester, find.text('Accept'));
  }

  Future<void> hangUp() async {
    if (!state.inCallPage) state.showCall();
    await _wait(tester, const Duration(milliseconds: 600));
    await _tap(tester, find.text('Hang up'));
  }

  // Ana calls Bea, then rings Carla (and Dani) in.
  if (_role == 'a') {
    await _wait(tester, const Duration(seconds: 3));
    await state.callFriend(named('b').id);
    await all(2, 'Bea answered');
    final add = find.byWidgetPredicate(
      (w) => w is Springy && w.tooltip == 'Add a friend to the call',
    );
    await _until(
      tester,
      'Ana may add people',
      () => add.evaluate().isNotEmpty,
      seconds: 30,
    );
    await _tap(tester, add);
    await _until(
      tester,
      'the friends to add',
      () => find.text('Add to the call').evaluate().isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'group-a-add');
    await _tap(tester, find.byKey(ValueKey('add-call:${named('c').id}')));
    await tester.tapAt(const Offset(30, 400));
    await _until(
      tester,
      'Carla rung in',
      () => find
          .byKey(ValueKey('calling-tile:${named('c').id}'))
          .evaluate()
          .isNotEmpty,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'group-a-calling');
    await all(3, 'Carla in');
    if (everyone == 4) {
      await state.addToCall(named('d').id);
      await all(4, 'Dani in');
    }
  } else if (_role == 'b') {
    await answer();
    await all(everyone, 'everyone in the call');
  } else {
    await answer();
    await all(everyone, 'everyone in the call');
  }
  await all(everyone, 'all $everyone in the call');
  // Bea and Carla are no friends, and see each other by name.
  final names = state.roomState.room!.participants
      .map((p) => p.person.name)
      .toSet();
  for (final r in _names.keys.take(everyone)) {
    if (!names.contains(_names[r])) throw StateError('${_names[r]} not seen');
  }
  final title = 'Call with ';
  if (find.textContaining(title).evaluate().isEmpty) {
    throw StateError('no call title');
  }
  await _wait(tester, const Duration(seconds: 1));
  await _shot(tester, 'group-$_role-in-call');
  await _say(state, 'all in');
  await _hear(tester, state, ['all in']);
  final group = state.callState.activeCall!.groupId;

  // Bea hangs up and comes back.
  if (_role == 'b') {
    await hangUp();
    await _until(tester, 'Bea out', () => state.callState.activeCall == null);
    await _wait(tester, const Duration(seconds: 4));
    await state.callFriend(named('a').id);
    await _until(
      tester,
      'Bea back in the same call',
      () =>
          state.callState.activeCall?.groupId == group &&
          state.callState.activeCall?.phase == UiCallPhase.connected,
      seconds: 60,
    );
  } else {
    await _until(
      tester,
      'Bea left and may come back',
      () =>
          state.callState.activeCall?.away.any((p) => p.name == 'Bea') ?? false,
      seconds: 60,
    );
    await _wait(tester, const Duration(milliseconds: 600));
    await _shot(tester, 'group-$_role-bea-left');
  }
  await all(everyone, 'Bea back');
  await _say(state, 'back');
  await _hear(tester, state, ['back']);

  // All but Carla hang up: she waits a minute, then the call ends.
  if (_role == 'c') {
    await _until(
      tester,
      'the last one left',
      () => state.callState.activeCall?.aloneSinceMs != null,
      seconds: 60,
    );
    await _wait(tester, const Duration(seconds: 2));
    await _shot(tester, 'group-c-alone');
    await _until(
      tester,
      'the call over after a minute alone',
      () =>
          state.callState.activeCall == null &&
          state.toasts.list.any(
            (t) => t.text == 'Nobody came back, so the call ended',
          ),
      seconds: 90,
    );
  } else {
    await _wait(tester, const Duration(seconds: 1));
    await hangUp();
    await _until(
      tester,
      'out of the call',
      () => state.callState.activeCall == null,
    );
  }
  _log('group call: added, came back, waited alone');
}

/// Each copy says what its microphone sends; each checks that the others' arrives. With a
/// tone on copy a, its mute and its change of microphone show on every other copy.
Future<void> _voice(WidgetTester tester, AppState state) async {
  final mine = switch (_mic) {
    '1' => 'tone',
    'system' => 'live',
    _ => 'muted',
  };
  await _say(state, 'voice $mine');
  final voices = await _hear(tester, state, [
    'voice tone',
    'voice live',
    'voice muted',
  ]);
  for (final MapEntry(key: other, value: said) in voices.entries) {
    final mode = said.split(' ').last;
    if (mode == 'muted') continue;
    final peer = _peer(state, other);
    // A tone keeps packets flowing; a real microphone in a quiet room sends one now and
    // then (Opus DTX).
    final least = mode == 'tone' ? 150 : 10;
    final start = await _received(
      state,
      'audio',
      'packetsReceived',
      only: peer,
    );
    await _wait(tester, const Duration(seconds: 6));
    final packets =
        await _received(state, 'audio', 'packetsReceived', only: peer) - start;
    _log('audio packets from ${other.name} in 6 s: $packets');
    if (packets < least) {
      throw StateError('too little audio from ${other.name}: $packets');
    }
    if (mode == 'tone') {
      await _until(
        tester,
        '${other.name} heard speaking',
        () => peer.speaking,
        seconds: 30,
      );
    } else {
      // A real microphone may pick up nothing; just note whether someone was heard.
      final end = DateTime.now().add(const Duration(seconds: 10));
      while (!peer.speaking && DateTime.now().isBefore(end)) {
        await _wait(tester, const Duration(milliseconds: 200));
      }
      _log(
        peer.speaking
            ? '${other.name} heard speaking'
            : 'nothing heard from ${other.name} in 10 s',
      );
    }
  }
  await _quality(state, 'voice');
  await _say(state, 'heard');
  await _hear(tester, state, ['heard']);

  final ana = voices.entries
      .where((e) => e.key.name == _names['a'])
      .firstOrNull;
  if ((_role == 'a' ? mine : ana?.value.split(' ').last) != 'tone') return;
  if (_role == 'a') {
    await _tap(tester, find.byTooltip('Mute microphone'));
    await _say(state, 'muted');
    await _hear(tester, state, ['saw muted']);
    await _tap(tester, find.byTooltip('Unmute microphone'));
    await _say(state, 'unmuted');
    await _hear(tester, state, ['heard again']);

    await state.setAudioInput(_quiet);
    await _say(state, 'quiet');
    await _hear(tester, state, ['saw quiet']);
    await state.setAudioInput(_tone);
    await _say(state, 'tone');
    await _hear(tester, state, ['heard tone']);
  } else {
    final peer = _peer(state, ana!.key);
    final name = ana.key.name;
    await _hearFrom(tester, state, ana.key, 'muted');
    await _until(tester, 'sees $name muted', () => !peer.mic, seconds: 15);
    await _say(state, 'saw muted');
    await _hearFrom(tester, state, ana.key, 'unmuted');
    await _until(tester, 'sees $name unmuted', () => peer.mic, seconds: 20);
    await _until(tester, '$name heard again', () => peer.speaking);
    await _say(state, 'heard again');

    // Silent with the microphone still on: the new microphone is in use, not a mute.
    await _hearFrom(tester, state, ana.key, 'quiet');
    bool silent() => peer.mic && !peer.speaking;
    await _until(tester, '$name silent', silent, seconds: 15);
    await _wait(tester, const Duration(seconds: 2));
    if (!silent()) throw StateError('$name heard on the silent microphone');
    // Silence goes out as a packet now and then (Opus DTX), not 50 a second.
    final packets = await _received(
      state,
      'audio',
      'packetsReceived',
      only: peer,
    );
    final bytes = await _received(state, 'audio', 'bytesReceived', only: peer);
    await _wait(tester, const Duration(seconds: 4));
    final quietPackets =
        await _received(state, 'audio', 'packetsReceived', only: peer) -
        packets;
    final quietBytes =
        await _received(state, 'audio', 'bytesReceived', only: peer) - bytes;
    _log(
      'audio from $name while silent: $quietPackets packets, '
      '$quietBytes bytes in 4 s',
    );
    if (quietPackets > 50) {
      throw StateError('$name sends silence in full: $quietPackets packets');
    }
    await _say(state, 'saw quiet');
    await _hearFrom(tester, state, ana.key, 'tone');
    await _until(tester, '$name heard on the tone', () => peer.speaking);

    // Her voice turned down on this device only: nothing of it plays here, and it
    // comes back at 100 %.
    await state.setVolume(ana.key.id, 0);
    await _until(
      tester,
      '$name silent here',
      () => !peer.speaking,
      seconds: 15,
    );
    await state.setVolume(ana.key.id, 100);
    await _until(tester, '$name heard here again', () => peer.speaking);
    await _say(state, 'heard tone');
  }
}

/// Counts the frames of the others' video for a few seconds and logs their size.
Future<void> _frames(
  WidgetTester tester,
  AppState state,
  String what,
  RTCVideoRenderer Function(CallPeer) view,
) async {
  String sizes() => state.callState.call!.peers.values
      .map(
        (p) =>
            '${p.id.substring(0, 8)} ${view(p).videoWidth}x${view(p).videoHeight}',
      )
      .join(', ');
  final start = await _received(state, 'video', 'framesDecoded');
  await _wait(tester, const Duration(seconds: 3));
  final frames = await _received(state, 'video', 'framesDecoded') - start;
  _log('$what: $frames frames in 3 s, ${sizes()}');
  if (frames < 3) throw StateError('too few frames of the $what: $frames');
  // How the picture settles while the connection finds its bandwidth.
  for (var seconds = 8; seconds <= 33; seconds += 5) {
    await _wait(tester, const Duration(seconds: 5));
    await _quality(state, '$what after $seconds s');
  }
}

/// While [other]'s screen is watched, another page is shown: the screen floats over it,
/// keeps arriving on the same connection and renderer, can be dragged, and a click on it
/// goes back to the room, where it fills the room again. Pictures `pip-<role>-*`.
Future<void> _floating(
  WidgetTester tester,
  AppState state,
  _Other other,
  CallPeer peer,
) async {
  final pip = find.byKey(const ValueKey('pip'));
  final role = _env['FOGATA_ROLE'];
  Future<int> videos() async => [
    for (final report
        in await peer.connection?.getStats() ?? const <StatsReport>[])
      if (report.type == 'inbound-rtp' && report.values['kind'] == 'video')
        report,
  ].length;
  final connection = peer.connection;
  final channels = await videos();
  state.goHome();
  await _until(
    tester,
    'floating picture shown',
    () => pip.evaluate().isNotEmpty,
    seconds: 10,
  );
  final start = await _received(state, 'video', 'framesDecoded', only: peer);
  await _wait(tester, const Duration(seconds: 3));
  final frames =
      await _received(state, 'video', 'framesDecoded', only: peer) - start;
  final size = tester.getSize(pip);
  _log(
    'floating: $frames frames in 3 s, ${size.width.round()}x'
    '${size.height.round()}, picture ${peer.screen.videoWidth}x'
    '${peer.screen.videoHeight}',
  );
  if (frames < 3) throw StateError('too few frames while floating: $frames');
  if (peer.connection != connection || await videos() != channels) {
    throw StateError('the floating picture came another way');
  }
  if (find.text('${other.name}’s screen').evaluate().length != 1) {
    throw StateError('the floating picture does not say whose it is');
  }
  await _shot(tester, 'pip-$role-home');
  // Dragged over the page, it stays in the window.
  await tester.drag(pip, const Offset(-500, -320));
  await _wait(tester, const Duration(milliseconds: 600));
  final window = tester.getSize(find.byType(Shell));
  final moved = tester.getRect(pip);
  _log('floating dragged to $moved in $window');
  if (moved.left < 0 ||
      moved.top < 40 ||
      moved.right > window.width ||
      moved.bottom > window.height) {
    throw StateError('dragged out of the window: $moved');
  }
  state.go(Screen.friends);
  await _wait(tester, const Duration(milliseconds: 800));
  if (pip.evaluate().isEmpty) {
    throw StateError('no floating picture on Friends');
  }
  await _shot(tester, 'pip-$role-dragged');
  await tester.tap(pip);
  await _until(
    tester,
    'back in the room from the floating picture',
    () =>
        state.nav.page == Screen.room &&
        pip.evaluate().isEmpty &&
        find.text('Stop watching').evaluate().isNotEmpty,
    seconds: 10,
  );
  if (state.callState.call?.watching != other.id) {
    throw StateError('stopped watching on the way back');
  }
  await _wait(tester, const Duration(milliseconds: 600));
  await _shot(tester, 'pip-$role-back');
}

/// Waits for [word] from every other copy while logging what this copy sends.
Future<void> _hearWhileSending(
  WidgetTester tester,
  AppState state,
  String word,
  String what,
) async {
  final end = DateTime.now().add(const Duration(seconds: 180));
  while (_said(state, [word]).length < _copies - 1) {
    if (DateTime.now().isAfter(end)) throw StateError('not in time: $word');
    await _wait(tester, const Duration(seconds: 5));
    await _quality(state, 'sending $what');
  }
  _log('heard from everyone: $word');
}

/// Copies with a camera turn it on; the others see it, then it goes off again.
Future<void> _cameras(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  if (_camera) {
    await state.setCamera(true);
    if (!call.camera) throw StateError('no camera: ${call.problem}');
  }
  await _say(state, _camera ? 'camera' : 'no camera');
  final cameras = [
    for (final MapEntry(key: other, value: said) in (await _hear(
      tester,
      state,
      ['camera', 'no camera'],
    )).entries)
      if (said == 'camera') other,
  ];
  for (final other in cameras) {
    final peer = _peer(state, other);
    await _until(
      tester,
      "${other.name}'s camera shown",
      () => peer.cam && peer.camera.videoWidth > 0,
      seconds: 60,
    );
  }
  if (cameras.isNotEmpty) {
    await _frames(tester, state, 'camera', (p) => p.camera);
    await _say(state, 'saw camera');
  }
  if (_camera) {
    await _hearWhileSending(tester, state, 'saw camera', 'camera');
    await state.setCamera(false);
  }
  await _say(state, 'camera done');
  await _hear(tester, state, ['camera done']);
}

/// Runs [work] while asking the platform thread for the video library's counts, one
/// question after the other, and logs the longest wait for an answer: a start or stop
/// that holds the platform thread (the window frozen meanwhile) shows as a long one.
Future<T> _whileAnswering<T>(String what, Future<T> Function() work) async {
  var longest = Duration.zero;
  var asked = 0;
  var done = false;
  final asking = () async {
    while (!done) {
      final wait = Stopwatch()..start();
      await _held();
      if (wait.elapsed > longest) longest = wait.elapsed;
      asked++;
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
  }();
  final took = Stopwatch()..start();
  try {
    return await work();
  } finally {
    done = true;
    await asking;
    _log(
      '$what in ${took.elapsedMilliseconds} ms; the platform thread answered '
      '$asked times, the longest after ${longest.inMilliseconds} ms',
    );
  }
}

/// The copy that shares shares [source] and stops [times] times, each start and stop
/// timed by the platform thread's answers. On Windows it then asks for two captures
/// with sound at once, as a second share asked for while the first one's sound still
/// starts: both must be answered (the first may go without its sound), and letting go
/// of both must leave no track behind.
Future<void> _shareAgain(
  Call call,
  DesktopCapturerSource source,
  int times,
) async {
  for (var i = 1; i <= times; i++) {
    await _whileAnswering('share $i started', () => call.shareScreen(source));
    if (!call.sharing) {
      throw StateError('share $i: not sharing: ${call.problem}');
    }
    _log('share $i: sound ${call.canShareSound}');
    await _whileAnswering('share $i stopped', () => call.stopSharing());
  }
  if (!Platform.isWindows) return;
  final before = await _held();
  final constraints = {
    'audio': true,
    'video': {
      'deviceId': {'exact': source.id},
      'mandatory': {'frameRate': 30.0},
    },
  };
  final both = await _whileAnswering(
    'two shares at once',
    () => Future.wait([
      navigator.mediaDevices.getDisplayMedia(constraints),
      navigator.mediaDevices.getDisplayMedia(constraints),
    ]),
  );
  _log(
    'two shares at once: sound tracks '
    '${both.map((s) => s.getAudioTracks().length).join(' and ')}',
  );
  for (final stream in both) {
    for (final track in stream.getTracks()) {
      await track.stop();
    }
    await stream.dispose();
  }
  final after = await _held();
  _log('held before $before, after $after');
  if ((after['tracks'] ?? 0) > (before['tracks'] ?? 0)) {
    throw StateError('two shares at once left tracks behind');
  }
}

/// Copies that share their screen do; the others see it come and go.
Future<void> _screens(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  if (_share != null) {
    final sources = await call.screens();
    _log(
      'screens: ${sources.map((s) => '${s.type.name}:${s.name}').join(' | ')}',
    );
    final source = sources.firstWhere(
      (s) => _share == 'window'
          ? s.type == SourceType.Window &&
                s.name.toLowerCase().contains('fogata')
          : s.type == SourceType.Screen,
      orElse: () => throw StateError('nothing to share for $_share'),
    );
    final again = int.tryParse(_env['FOGATA_TEST_SCREEN_AGAIN'] ?? '');
    if (again != null) await _shareAgain(call, source, again);
    await _whileAnswering('share started', () => call.shareScreen(source));
    if (!call.sharing) throw StateError('not sharing: ${call.problem}');
    _log('share: sound ${call.canShareSound}');
  }
  await _say(state, _share != null ? 'screen' : 'no screen');
  final sharers = [
    for (final MapEntry(key: other, value: said) in (await _hear(
      tester,
      state,
      ['screen', 'no screen'],
    )).entries)
      if (said == 'screen') other,
  ];
  // A shared screen shows as a card and is only sent here once watched: one at a time.
  for (final other in sharers) {
    final peer = _peer(state, other);
    await _until(
      tester,
      "${other.name}'s screen offered",
      () => peer.sharing && find.text('Watch').evaluate().isNotEmpty,
      seconds: 60,
    );
    if (!_camera && call.peers.values.every((p) => !p.cam)) {
      final before = await _received(state, 'video', 'framesDecoded');
      await _wait(tester, const Duration(seconds: 2));
      final unwatched =
          await _received(state, 'video', 'framesDecoded') - before;
      _log('frames while not watching: $unwatched');
      if (unwatched > 0) throw StateError('a screen came before watching it');
    }
    call.watch(other.id);
    await _until(
      tester,
      "${other.name}'s screen shown",
      () =>
          peer.screen.videoWidth > 0 &&
          find.textContaining('${other.name}’s screen').evaluate().isNotEmpty,
      seconds: 60,
    );
    await _frames(tester, state, 'screen', (p) => p.screen);
    if (_env['FOGATA_SHOTS'] != null) {
      // The pointer on the picture shows its controls; still, they go after 2.5 s.
      final role = _env['FOGATA_ROLE'];
      final title = find.textContaining('${other.name}’s screen');
      final picture = tester.getCenter(title.first) + const Offset(0, 220);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: picture);
      await mouse.moveBy(const Offset(4, 4));
      await _wait(tester, const Duration(seconds: 3));
      await mouse.moveBy(const Offset(4, 4));
      await _wait(tester, const Duration(milliseconds: 600));
      await _shot(tester, 'watch-$role-shown');
      final quality = find.textContaining(RegExp(r'^(Good|Okay|Poor)$'));
      _log('quality shown: ${quality.evaluate().isNotEmpty}');
      if (quality.evaluate().isNotEmpty) {
        await mouse.moveTo(tester.getCenter(quality.first));
        await _wait(tester, const Duration(milliseconds: 700));
        await _shot(tester, 'watch-$role-quality');
      }
      await mouse.moveTo(picture);
      await _wait(tester, const Duration(milliseconds: 3500));
      await _shot(tester, 'watch-$role-hidden');
      await mouse.removePointer();
    }
    await _floating(tester, state, other, peer);
    if (peer.sound) {
      // The other applications' sound comes with the screen, on its own channel or (from
      // macOS) on a connection of its own, which takes a moment to open.
      var packets = 0;
      var level = 0.0;
      for (var i = 0; i < 30 && packets == 0; i++) {
        if (i > 0) await _wait(tester, const Duration(milliseconds: 500));
        final reports = [
          for (final report in await peer.connection!.getStats())
            if (report.values['mid'] == '3') report,
          ...?await peer.soundConnection?.getStats(),
        ];
        for (final report in reports) {
          final v = report.values;
          if (report.type == 'inbound-rtp' && v['kind'] == 'audio') {
            final got = (v['packetsReceived'] as num? ?? 0).toInt();
            if (got > packets) {
              packets = got;
              level = (v['audioLevel'] as num? ?? 0).toDouble();
            }
          }
        }
      }
      _log('screen sound from ${other.name}: $packets packets, level $level');
      if (packets == 0) throw StateError('no sound with the screen');
    } else {
      _log('${other.name} shares no sound');
    }
    call.watch(null);
    await _wait(tester, const Duration(milliseconds: 500));
  }
  if (sharers.isNotEmpty) await _say(state, 'saw screen');
  if (_share != null) {
    await _hearWhileSending(tester, state, 'saw screen', 'screen');
    // This device's own screen can fill the room too.
    call.showOwnScreen(true);
    await _until(
      tester,
      'own screen enlarged',
      () => find.text('Back').evaluate().isNotEmpty,
      seconds: 10,
    );
    await _wait(tester, const Duration(seconds: 2));
    call.showOwnScreen(false);
    await _whileAnswering(
      'share stopped',
      () => _tap(tester, find.byTooltip('Stop sharing the screen')),
    );
    if (call.sharing) throw StateError('still sharing');
    await _say(state, 'stopped sharing');
  } else {
    await _say(state, 'not sharing');
  }
  if (sharers.isNotEmpty) {
    final stopped = await _hear(tester, state, [
      'stopped sharing',
      'not sharing',
    ]);
    for (final other in sharers) {
      final peer = _peer(state, other);
      if (stopped[other] != 'stopped sharing') continue;
      await _until(
        tester,
        "${other.name}'s screen gone",
        () =>
            !peer.sharing &&
            find.textContaining('${other.name}’s screen').evaluate().isEmpty,
        seconds: 30,
      );
    }
  }
}

/// The size and frame rate of the screen arriving from [peer] now.
Future<({int width, int height, double fps})> _screenIn(CallPeer peer) async {
  for (final report in await peer.connection!.getStats()) {
    final v = report.values;
    if (report.type == 'inbound-rtp' &&
        v['kind'] == 'video' &&
        '${v['mid']}' == '2') {
      return (
        width: (v['frameWidth'] as num? ?? 0).toInt(),
        height: (v['frameHeight'] as num? ?? 0).toInt(),
        fps: (v['framesPerSecond'] as num? ?? 0).toDouble(),
      );
    }
  }
  return (width: 0, height: 0, fps: 0.0);
}

/// The packets of [peer]'s screen sound received so far, on its channel or (from macOS) on
/// a connection of its own.
Future<int> _screenSoundIn(CallPeer peer) async {
  var packets = 0;
  final reports = [
    for (final report in await peer.connection!.getStats())
      if (report.values['mid'] == '3') report,
    ...?await peer.soundConnection?.getStats(),
  ];
  for (final report in reports) {
    final v = report.values;
    if (report.type == 'inbound-rtp' && v['kind'] == 'audio') {
      packets = math.max(packets, (v['packetsReceived'] as num? ?? 0).toInt());
    }
  }
  return packets;
}

/// This copy's shared screen as captured and as sent to [peer]: sizes and frame rates.
Future<String> _screenOut(CallPeer peer) async {
  final parts = <String>[];
  for (final report in await peer.connection!.getStats()) {
    final v = report.values;
    if (report.type == 'media-source' && v['kind'] == 'video') {
      parts.add(
        'source ${v['width']}x${v['height']} at ${v['framesPerSecond']} fps',
      );
    } else if (report.type == 'outbound-rtp' && '${v['mid']}' == '2') {
      parts.add(
        'sent ${v['frameWidth']}x${v['frameHeight']} at '
        '${v['framesPerSecond']} fps, limited by ${v['qualityLimitationReason']}',
      );
    }
  }
  return parts.join('; ');
}

/// The rate this copy's screen is captured at, from the first connection's statistics.
Future<double> _capturedFps(CallPeer peer) async {
  var fps = 0.0;
  for (final report in await peer.connection!.getStats()) {
    final v = report.values;
    if (report.type == 'media-source' && v['kind'] == 'video') {
      fps = math.max(fps, (v['framesPerSecond'] as num? ?? 0).toDouble());
    }
  }
  return fps;
}

/// One copy shares its screen again and, while the others watch it, changes how it goes
/// out: 720p at 15 frames a second, then 1080p at 60. The others check the size that
/// arrives and that no more than 15 frames a second do; the sharer checks the capture
/// was asked for each rate (how many a busy computer manages varies).
Future<void> _screenQuality(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final steps = [
    ('720p 15', const ScreenQuality(lines: 720, fps: 15)),
    ('1080p 60', const ScreenQuality(lines: 1080, fps: 60)),
  ];
  // The others first see the last share end.
  await _say(state, 'screens done');
  await _hear(tester, state, ['screens done']);
  DesktopCapturerSource? source;
  if (_share != null) {
    await state.setScreenQuality(const ScreenQuality());
    final sources = await call.screens();
    source = sources.firstWhere(
      (s) => _share == 'window'
          ? s.type == SourceType.Window &&
                s.name.toLowerCase().contains('fogata')
          : s.type == SourceType.Screen,
    );
    await call.shareScreen(source);
    if (!call.sharing) throw StateError('not sharing: ${call.problem}');
  }
  await _say(state, _share != null ? 'quality screen' : 'quality no screen');
  final said = await _hear(tester, state, [
    'quality screen',
    'quality no screen',
  ]);
  final sharers = [
    for (final MapEntry(key: other, value: word) in said.entries)
      if (word == 'quality screen') other,
  ];
  // Only with one copy sharing: the others all watch it.
  if (sharers.length + (_share != null ? 1 : 0) != 1) {
    _log('screen quality skipped: not exactly one copy shares');
  } else if (_share != null) {
    final peer = call.peers.values.first;
    for (final (step, quality) in steps) {
      await _hear(tester, state, ['ready $step']);
      await state.setScreenQuality(quality);
      await _say(state, 'quality $step');
      for (var i = 0; i < 4; i++) {
        await _wait(tester, const Duration(seconds: 2));
        _log('screen out at $step: ${await _screenOut(peer)}');
      }
      // What a busy computer manages varies; the capture must at least be asked for
      // the new rate.
      _log(
        'captured at $step: asked ${call.capturedFps}, '
        'got ${await _capturedFps(peer)} fps',
      );
      if (call.capturedFps != quality.fps) {
        throw StateError('not captured at $step: ${call.capturedFps} fps');
      }
      // What the quality card says each one watching gets, from the same readings.
      final out = call.screenOut.value[peer.id];
      _log('going out at $step: $out');
      if (out == null || out.height > quality.lines!) {
        throw StateError('going out at $step, the card says $out');
      }
      await _hearWhileSending(tester, state, 'saw $step', step);
    }
    // Another screen picked while it is watched (the same one again here): it goes out
    // scaled from its first picture on, so one larger than 8K never gets this copy kept
    // out of the call.
    await _hear(tester, state, ['ready to switch']);
    await call.shareScreen(source);
    if (!call.sharing) throw StateError('not sharing: ${call.problem}');
    _log('picked the screen again: ${await _screenOut(peer)}');
    await _say(state, 'switched');
    await _hearWhileSending(tester, state, 'saw the switch', 'switch');
  } else {
    final sharer = sharers.single;
    final peer = _peer(state, sharer);
    call.watch(sharer.id);
    await _until(
      tester,
      "${sharer.name}'s screen shown again",
      () => peer.sharing && peer.screen.videoWidth > 0,
      seconds: 60,
    );
    final first = await _screenIn(peer);
    _log('screen at first: ${first.width}x${first.height} ${first.fps} fps');
    for (final (step, quality) in steps) {
      await _say(state, 'ready $step');
      await _hearFrom(tester, state, sharer, 'quality $step');
      final lines = quality.lines!;
      var now = await _screenIn(peer);
      // The size changes with the next key picture; the rate settles over a few seconds.
      final end = DateTime.now().add(const Duration(seconds: 20));
      while (DateTime.now().isBefore(end) &&
          (now.height > lines ||
              (lines >= first.height && now.height < first.height))) {
        await _wait(tester, const Duration(milliseconds: 500));
        now = await _screenIn(peer);
      }
      await _wait(tester, const Duration(seconds: 4));
      var fps = 0.0;
      for (var i = 0; i < 5; i++) {
        await _wait(tester, const Duration(seconds: 1));
        now = await _screenIn(peer);
        fps += now.fps / 5;
      }
      _log(
        'screen at $step: ${now.width}x${now.height}, ${fps.toStringAsFixed(1)} fps',
      );
      if (now.height > lines || now.width > lines * 16 / 9) {
        throw StateError(
          'screen larger than $step: ${now.width}x${now.height}',
        );
      }
      if (quality.fps < 30 && fps > quality.fps + 3) {
        throw StateError('screen faster than $step: $fps fps');
      }
      // The screen was captured again for the new rate: its sound comes still.
      if (peer.sound) {
        final before = await _screenSoundIn(peer);
        await _wait(tester, const Duration(seconds: 3));
        final after = await _screenSoundIn(peer);
        _log('screen sound at $step: ${after - before} packets in 3 s');
        if (after <= before) throw StateError('no screen sound at $step');
      }
      await _say(state, 'saw $step');
    }
    await _say(state, 'ready to switch');
    await _hearFrom(tester, state, sharer, 'switched');
    final end = DateTime.now().add(const Duration(seconds: 10));
    var now = await _screenIn(peer);
    while (DateTime.now().isBefore(end) && !peer.keptOut) {
      await _wait(tester, const Duration(milliseconds: 500));
      now = await _screenIn(peer);
    }
    if (peer.keptOut) {
      throw StateError(
        'kept ${sharer.name} out after another screen was picked',
      );
    }
    _log('screen after the switch: ${now.width}x${now.height}');
    if (now.width <= 0) throw StateError('no screen after the switch');
    await _say(state, 'saw the switch');
    call.watch(null);
  }
  if (_share != null) {
    await state.setScreenQuality(const ScreenQuality());
    await call.stopSharing();
  }
  await _say(state, 'quality done');
  await _hear(tester, state, ['quality done']);
}

/// The statistics of a connection this test follows: of the voice (`voice.`), screen
/// (`screen.`) and screen sound (`sound.`) channels, in or out, of the path in use
/// (`path.`), of the other side's reports on the screen it got (`remote.`), and how many
/// candidate pairs work and how often the one in use changed (`transport.`).
///
/// The call can let a connection go after a long outage and make a new one: the one in use
/// is looked up each time (`connection` tells them apart), and none gives no statistics.
Future<Map<String, Object?>> _watchStats(AppState state, String id) async {
  final connection = state.callState.call?.peers[id]?.connection;
  if (connection == null) return {};
  final reports = await connection.getStats();
  final values = <String, Object?>{'connection': identityHashCode(connection)};
  String? selected;
  var pairs = 0;
  for (final report in reports) {
    if (report.type == 'transport') {
      selected = report.values['selectedCandidatePairId'] as String?;
      values['transport.pairChanges'] =
          report.values['selectedCandidatePairChanges'];
    }
    if (report.type == 'candidate-pair' &&
        report.values['state'] == 'succeeded') {
      pairs++;
    }
  }
  values['transport.pairs'] = pairs;
  final codecs = {
    for (final report in reports)
      if (report.type == 'codec') report.id: report.values['mimeType'],
  };
  for (final report in reports) {
    final v = report.values;
    // The voice codec each way, as WebRTC names it: Opus also when RED carries it (then
    // the packets are about twice as large).
    if (v['mid'] == '0' || v['mid'] == 0) {
      if (report.type == 'outbound-rtp') {
        values['voice.out'] = codecs[v['codecId']];
      }
      if (report.type == 'inbound-rtp') {
        values['voice.in'] = codecs[v['codecId']];
      }
    }
    final channel = switch ('${v['mid']}') {
      '0' => 'voice',
      '1' => 'camera',
      '2' => 'screen',
      '3' => 'sound',
      _ => null,
    };
    final prefix = switch (report.type) {
      'inbound-rtp' || 'outbound-rtp' when channel != null => '$channel.',
      'remote-inbound-rtp' when v['kind'] == 'video' => 'remote.',
      'candidate-pair'
          when report.id == selected ||
              (selected == null && v['nominated'] == true) =>
        'path.',
      _ => null,
    };
    if (prefix == null) continue;
    for (final MapEntry(:key, :value) in v.entries) {
      values['$prefix$key'] = value;
    }
  }
  return values;
}

/// Logs the codecs of the voice section of both descriptions, in their order: the first
/// one each side supports is what it sends.
Future<void> _logVoiceCodecs(CallPeer peer) async {
  String codecs(String? sdp) {
    final lines = (sdp ?? '').split('\r\n');
    final at = lines.indexOf('a=mid:0');
    if (at < 0) return '?';
    final start = lines.lastIndexWhere((l) => l.startsWith('m='), at);
    final names = {
      for (final line
          in lines.skip(start + 1).takeWhile((l) => !l.startsWith('m=')))
        if (RegExp(r'^a=rtpmap:(\d+) (\S+)').firstMatch(line) case final m?)
          m.group(1)!: m.group(2)!,
    };
    return lines[start].split(' ').skip(3).map((t) => names[t] ?? t).join(' ');
  }

  final connection = peer.connection!;
  final local = await connection.getLocalDescription();
  final remote = await connection.getRemoteDescription();
  _log(
    'voice codecs: here ${codecs(local?.sdp)}; there ${codecs(remote?.sdp)}',
  );
}

/// The middle of [values], and the ones a tenth of them lie under and over.
({double median, double low, double high}) _spread(List<double> values) {
  if (values.isEmpty) {
    return (median: double.nan, low: double.nan, high: double.nan);
  }
  final sorted = [...values]..sort();
  double at(double q) => sorted[((sorted.length - 1) * q).round()];
  return (median: at(0.5), low: at(0.1), high: at(0.9));
}

/// After 15 s of voice, the copy with FOGATA_TEST_SCREEN shares its screen and the
/// others watch it for [seconds]. Every 2 s each viewer logs the size, frame rate, delay (half the round trip
/// and the time in the jitter buffer), losses and freezes of the screen, and how the voice
/// comes through next to it; the sharer logs what it sends and what WebRTC thinks it can
/// send. Each ends with a "watch summary" line.
Future<void> _watchScreen(
  WidgetTester tester,
  AppState state,
  int seconds,
) async {
  final call = state.callState.call!;
  // Like a call where people talk a while before someone shares.
  await _wait(tester, const Duration(seconds: 15));
  await _quality(state, 'before sharing');
  if (_share != null) {
    final sources = await call.screens();
    await call.shareScreen(
      sources.firstWhere(
        (s) => _share == 'window'
            ? s.type == SourceType.Window &&
                  s.name.toLowerCase().contains('fogata')
            : s.type == SourceType.Screen,
      ),
    );
    if (!call.sharing) throw StateError('not sharing: ${call.problem}');
  }
  await _say(state, _share != null ? 'watch screen' : 'watch no screen');
  final said = await _hear(tester, state, ['watch screen', 'watch no screen']);
  final sharers = [
    for (final MapEntry(key: other, value: word) in said.entries)
      if (word == 'watch screen') other,
  ];
  if (sharers.length + (_share != null ? 1 : 0) != 1) {
    throw StateError('exactly one copy must share its screen');
  }
  await _logVoiceCodecs(call.peers.values.first);
  String ms(double v) => v.isNaN ? '?' : '${v.round()} ms';
  // Counts read from stats that may be missing for a moment (an outage, a new connection).
  String count(double v) => v.isNaN ? '?' : '${v.round()}';
  String num1(double v) => v.isNaN ? '?' : v.toStringAsFixed(1);
  double d(Object? v) => (v as num? ?? double.nan).toDouble();

  if (_share != null) {
    final peer = call.peers.keys.first;
    final fps = <double>[], target = <double>[], sent = <double>[];
    final canSend = <double>[], sizes = <String, int>{};
    final voiceSent = <double>[], voiceSizes = <double>[];
    var before = await _watchStats(state, peer);
    var at = DateTime.now();
    final end = DateTime.now().add(Duration(seconds: seconds + 240));
    while (_said(state, ['watched']).length < _copies - 1) {
      if (DateTime.now().isAfter(end)) throw StateError('not in time: watched');
      await _wait(tester, const Duration(seconds: 2));
      final now = await _watchStats(state, peer);
      final took = DateTime.now().difference(at).inMilliseconds / 1000;
      at = DateTime.now();
      // Counts of another connection than the last reading's do not compare.
      if (now['connection'] != before['connection']) before = {};
      double delta(String key) => d(now[key]) - d(before[key]);
      final frames = delta('screen.framesSent') / took;
      final kbps = delta('screen.bytesSent') * 8 / took / 1000;
      final rtx = delta('screen.retransmittedBytesSent') * 8 / took / 1000;
      final voice = delta('voice.bytesSent') * 8 / took / 1000;
      final voicePackets = delta('voice.packetsSent');
      final voiceBytes = delta('voice.bytesSent') / voicePackets;
      final sound = delta('sound.bytesSent') * 8 / took / 1000;
      final size = '${now['screen.frameWidth']}x${now['screen.frameHeight']}';
      if (!frames.isNaN) fps.add(frames);
      if (!kbps.isNaN) sent.add(kbps);
      if (!voice.isNaN) voiceSent.add(voice);
      if (!voiceBytes.isNaN) voiceSizes.add(voiceBytes);
      target.add(d(now['screen.targetBitrate']) / 1000);
      canSend.add(d(now['path.availableOutgoingBitrate']) / 1000);
      sizes[size] = (sizes[size] ?? 0) + 1;
      _log(
        'watch out: $size at ${num1(frames)} fps, '
        'target ${num1(target.last)} kbit/s, sent ${num1(kbps)} kbit/s '
        '(resent ${num1(rtx)}), can send ${num1(canSend.last)} kbit/s, '
        'round trip ${ms(d(now['path.currentRoundTripTime']) * 1000)}, '
        'other side lost ${num1(d(now['remote.fractionLost']) * 100)} %, '
        'pairs ${now['transport.pairs']}, '
        '${now['transport.pairChanges']} changes of pair, '
        'limited by ${now['screen.qualityLimitationReason']} '
        '(${now['screen.qualityLimitationResolutionChanges']} changes, '
        '${now['screen.contentType'] ?? 'no content type'}); '
        'voice ${num1(voice)} kbit/s (${now['voice.out']}, '
        '${count(voicePackets)} packets of ${num1(voiceBytes)} bytes), '
        'screen sound ${num1(sound)} kbit/s',
      );
      before = now;
    }
    final f = _spread(fps), t = _spread(target), c = _spread(canSend);
    final s = _spread(sent);
    final size = sizes.entries.reduce((a, b) => a.value >= b.value ? a : b).key;
    _log(
      'watch summary out: mostly $size; fps median ${num1(f.median)}, '
      'low ${num1(f.low)}; target median ${num1(t.median)} kbit/s; '
      'sent median ${num1(s.median)} kbit/s; '
      'can send median ${num1(c.median)}, low ${num1(c.low)} kbit/s; '
      'voice ${before['voice.out']} median ${num1(_spread(voiceSent).median)} kbit/s, '
      '${num1(_spread(voiceSizes).median)} bytes a packet',
    );
    await call.stopSharing();
    return;
  }

  final sharer = sharers.single;
  final peer = _peer(state, sharer);
  final asked = DateTime.now();
  call.watch(sharer.id);
  await _until(
    tester,
    "${sharer.name}'s screen shown",
    () => peer.sharing && peer.screen.videoWidth > 0,
    seconds: 120,
  );
  _log(
    'first picture after ${DateTime.now().difference(asked).inMilliseconds} ms',
  );
  final fps = <double>[], delay = <double>[], voiceDelay = <double>[];
  final sizes = <String, int>{};
  var concealed = 0.0, samples = 0.0;
  final first = await _watchStats(state, sharer.id);
  var before = first;
  var at = DateTime.now();
  final end = DateTime.now().add(Duration(seconds: seconds));
  while (DateTime.now().isBefore(end)) {
    await _wait(tester, const Duration(seconds: 2));
    final now = await _watchStats(state, sharer.id);
    final took = DateTime.now().difference(at).inMilliseconds / 1000;
    at = DateTime.now();
    // Counts of another connection than the last reading's do not compare.
    if (now['connection'] != before['connection']) before = {};
    double delta(String key) => d(now[key]) - d(before[key]);
    final frames = delta('screen.framesDecoded') / took;
    final kbps = delta('screen.bytesReceived') * 8 / took / 1000;
    final buffered =
        delta('screen.jitterBufferDelay') /
        delta('screen.jitterBufferEmittedCount') *
        1000;
    // From sending to showing, about: half the round trip, then the time in the jitter
    // buffer (packets that come late make it hold everything longer).
    final path = d(now['path.currentRoundTripTime']) * 1000 / 2;
    final late = path + buffered;
    final voiceLate =
        path +
        delta('voice.jitterBufferDelay') /
            delta('voice.jitterBufferEmittedCount') *
            1000;
    final voiceTarget =
        delta('voice.jitterBufferTargetDelay') /
        delta('voice.jitterBufferEmittedCount') *
        1000;
    final voiceKbps = delta('voice.bytesReceived') * 8 / took / 1000;
    // How evenly the pictures come: the mean time from one to the next, and its spread.
    final gaps = delta('screen.framesDecoded');
    final gap = delta('screen.totalInterFrameDelay') / gaps * 1000;
    final spread = math.sqrt(
      math.max(
        0,
        delta('screen.totalSquaredInterFrameDelay') / gaps * 1e6 - gap * gap,
      ),
    );
    final hidden = delta('voice.concealedSamples');
    final heard = delta('voice.totalSamplesReceived');
    final size = '${now['screen.frameWidth']}x${now['screen.frameHeight']}';
    fps.add(frames.isNaN ? 0 : frames);
    if (!late.isNaN) delay.add(late);
    if (!voiceLate.isNaN) voiceDelay.add(voiceLate);
    if (!hidden.isNaN && !heard.isNaN) {
      concealed += hidden;
      samples += heard;
    }
    sizes[size] = (sizes[size] ?? 0) + 1;
    _log(
      'watch in: $size at ${num1(frames)} fps, ${num1(kbps)} kbit/s, '
      'delay ${ms(late)} (buffer ${ms(buffered)}), '
      'lost ${count(delta('screen.packetsLost'))} of '
      '${count(delta('screen.packetsReceived') + delta('screen.packetsLost'))}, '
      'freezes ${count(d(now['screen.freezeCount']))}, '
      'frames every ${ms(gap)} ± ${ms(spread)}; voice: '
      '${count(delta('voice.packetsReceived'))} packets, '
      '${count(delta('voice.packetsLost'))} lost, '
      'concealed ${num1(heard > 0 ? hidden / heard * 100 : double.nan)} %, '
      'delay ${ms(voiceLate)} (target ${ms(voiceTarget)}), '
      '${num1(voiceKbps)} kbit/s ${now['voice.in']}; '
      'round trip ${ms(d(now['path.currentRoundTripTime']) * 1000)}',
    );
    before = now;
  }
  final f = _spread(fps), l = _spread(delay), v = _spread(voiceDelay);
  final size = sizes.entries.reduce((a, b) => a.value >= b.value ? a : b).key;
  _log(
    'watch summary in: mostly $size; fps median ${num1(f.median)}, '
    'low ${num1(f.low)}; under 5 fps ${fps.where((x) => x < 5).length} of '
    '${fps.length} samples; delay median ${ms(l.median)}, high ${ms(l.high)}; '
    'freezes ${count(d(before['screen.freezeCount']) - d(first['screen.freezeCount']))} '
    '(${num1(d(before['screen.totalFreezesDuration']) - d(first['screen.totalFreezesDuration']))} s); '
    'voice concealed ${num1(samples > 0 ? concealed / samples * 100 : double.nan)} %, '
    'delay median ${ms(v.median)}, high ${ms(v.high)}, ${before['voice.in']}',
  );
  call.watch(null);
  await _say(state, 'watched');
}

/// The processor time this process has used, in seconds; NaN where it cannot be read.
double _processorSeconds() {
  if (!Platform.isLinux) return double.nan;
  try {
    final stat = File('/proc/self/stat').readAsStringSync();
    final fields = stat.substring(stat.lastIndexOf(')') + 2).split(' ');
    // utime and stime, in clock ticks of 1/100 s.
    return (int.parse(fields[11]) + int.parse(fields[12])) / 100;
  } catch (_) {
    return double.nan;
  }
}

/// Every copy turns its camera on and keeps it on while the room has all the copies, then
/// only the first half (the others leave), then all of them again, [seconds] each time:
/// what each camera sends follows the number of people. See [_measureCameras] for what is
/// logged. The copies tell each other the steps with files in FOGATA_CALL, since a copy
/// out of the room does not get its chat.
Future<void> _cameraSteps(
  WidgetTester tester,
  AppState state,
  int seconds,
) async {
  final group = state.roomState.roomGroup!;
  final all = _names.keys.take(_copies).toList();
  final stay = all.take((_copies + 1) ~/ 2).toList();
  File mark(String name) => File('$_dir/cams-$name');
  Future<void> everyone(String step, List<String> roles) => _until(
    tester,
    'everyone: $step',
    () => roles.every((r) => mark('$step-$r').existsSync()),
    seconds: seconds + 600,
  );
  bool connectedTo(int others) {
    final call = state.callState.call;
    return call != null &&
        call.peers.length == others &&
        call.peers.values.every((p) => p.connected && p.cam);
  }

  await state.setCamera(true);
  if (state.callState.call?.camera != true) {
    throw StateError('no camera: ${state.callState.call?.problem}');
  }
  await _until(
    tester,
    'every camera on',
    () => connectedTo(all.length - 1),
    seconds: 120,
  );
  mark('on-$_role').writeAsStringSync('');
  await everyone('on', all);
  await _measureCameras(tester, state, 'all ${all.length}', seconds);
  mark('first-$_role').writeAsStringSync('');
  await everyone('first', all);
  if (!stay.contains(_role)) {
    await state.leaveRoom();
    _log('left the room');
    await _until(
      tester,
      'asked back',
      () => mark('back').existsSync(),
      seconds: seconds + 600,
    );
    // Out of a call, this only asks for the camera when joining.
    await state.setCamera(true);
    await state.openRoom(group);
    await _until(tester, 'in the call', () => state.callState.call != null);
    if (!state.callState.call!.camera) await state.setCamera(true);
  } else {
    await _until(
      tester,
      'the others left',
      () => connectedTo(stay.length - 1),
      seconds: 120,
    );
    await _measureCameras(tester, state, 'half ${stay.length}', seconds);
    if (_role == 'a') mark('back').writeAsStringSync('');
  }
  await _until(
    tester,
    'everyone back',
    () => connectedTo(all.length - 1),
    seconds: 300,
  );
  mark('again-$_role').writeAsStringSync('');
  await everyone('again', all);
  await _measureCameras(tester, state, 'back ${all.length}', seconds);
  mark('done-$_role').writeAsStringSync('');
  await everyone('done', all);
  await state.setCamera(false);
}

/// For [seconds], every 2 s, logs how this copy's camera goes out to each other copy (size,
/// frame rate, kbit/s sent, WebRTC's target and what limits it), the time spent encoding
/// it (in all its encoders, as a share of one core), the processor time of the whole
/// copy, and the freezes of the cameras it receives; then a summary of the readings after
/// the first 10 s, while WebRTC finds its rates. With FOGATA_TEST_CAMERA_PEOPLE, the
/// camera goes out as in a call of that many people.
Future<void> _measureCameras(
  WidgetTester tester,
  AppState state,
  String label,
  int seconds,
) async {
  final call = state.callState.call!;
  final people = int.tryParse(_env['FOGATA_TEST_CAMERA_PEOPLE'] ?? '');
  if (people != null) call.peopleForCamera = people;
  String num1(double v) => v.isNaN ? '?' : v.toStringAsFixed(1);
  double d(Object? v) => (v as num? ?? double.nan).toDouble();
  final totals = <double>[], each = <double>[], fps = <double>[];
  final encoding = <double>[], processor = <double>[], perFrame = <double>[];
  final sizes = <String, int>{};
  var freezes = 0.0;
  Map<String, Map<String, Object?>> before = {};
  var cpu = _processorSeconds();
  var at = DateTime.now();
  final start = at;
  final end = start.add(Duration(seconds: seconds));
  while (DateTime.now().isBefore(end)) {
    await _wait(tester, const Duration(seconds: 2));
    final now = <String, Map<String, Object?>>{};
    for (final peer in call.peers.values.toList()) {
      final connection = peer.connection;
      if (connection == null) continue;
      final values = <String, Object?>{
        'connection': identityHashCode(connection),
      };
      for (final report in await connection.getStats()) {
        final v = report.values;
        if ('${v['mid']}' != '1') continue;
        if (report.type == 'outbound-rtp') {
          for (final MapEntry(:key, :value) in v.entries) {
            values['out.$key'] = value;
          }
        } else if (report.type == 'inbound-rtp') {
          values['in.freezeCount'] = v['freezeCount'];
        }
      }
      now[peer.id] = values;
    }
    final took = DateTime.now().difference(at).inMilliseconds / 1000;
    at = DateTime.now();
    final cpuNow = _processorSeconds();
    final share = (cpuNow - cpu) / took * 100;
    cpu = cpuNow;
    var total = 0.0, encode = 0.0;
    final parts = <String>[];
    final settled = at.difference(start) >= const Duration(seconds: 10);
    for (final MapEntry(key: id, value: v) in now.entries) {
      var last = before[id] ?? const <String, Object?>{};
      // Counts of another connection than the last reading's do not compare.
      if (last['connection'] != v['connection']) last = const {};
      double delta(String key) => d(v[key]) - d(last[key]);
      final kbps = delta('out.bytesSent') * 8 / took / 1000;
      final frames = delta('out.framesEncoded') / took;
      final busy = delta('out.totalEncodeTime') / took * 100;
      final size = '${v['out.frameWidth']}x${v['out.frameHeight']}';
      if (!kbps.isNaN) total += kbps;
      if (!busy.isNaN) encode += busy;
      if (settled) {
        if (!kbps.isNaN) each.add(kbps);
        if (!frames.isNaN) fps.add(frames);
        sizes[size] = (sizes[size] ?? 0) + 1;
        final froze = delta('in.freezeCount');
        if (!froze.isNaN) freezes += froze;
        final ms =
            delta('out.totalEncodeTime') / delta('out.framesEncoded') * 1000;
        if (!ms.isNaN && ms.isFinite) perFrame.add(ms);
      }
      parts.add(
        '${id.substring(0, 8)} $size at ${num1(frames)} fps, '
        '${num1(kbps)} kbit/s (target '
        '${num1(d(v['out.targetBitrate']) / 1000)}, limited by '
        '${v['out.qualityLimitationReason']})',
      );
    }
    before = now;
    if (settled) {
      totals.add(total);
      encoding.add(encode);
      if (!share.isNaN) processor.add(share);
    }
    _log(
      'cameras $label: ${now.length + 1} people; sent ${num1(total)} kbit/s, '
      'encoding ${num1(encode)} %, process ${num1(share)} %; '
      '${parts.join('; ')}',
    );
  }
  final t = _spread(totals), e = _spread(each), f = _spread(fps);
  final size = sizes.isEmpty
      ? '?'
      : sizes.entries.reduce((a, b) => a.value >= b.value ? a : b).key;
  _log(
    'cameras summary $label: sent median ${num1(t.median)} kbit/s '
    '(${num1(t.low)} to ${num1(t.high)}), to each ${num1(e.median)}; '
    'mostly $size at ${num1(f.median)} fps (sizes $sizes); '
    'encoding median ${num1(_spread(encoding).median)} % of a core '
    '(${num1(_spread(perFrame).median)} ms a frame); '
    'process median ${num1(_spread(processor).median)} %; '
    'freezes received $freezes',
  );
}

/// After 20 s of voice only, copy a turns its camera on; for [seconds] from then, every
/// second, each other copy logs how a's camera arrives (size, frames a second, kbit/s,
/// packets lost, freezes) and copy a how it goes out to each (size, WebRTC's target and
/// what limits it, what the connection can send). Each ends with a "camera start" summary:
/// when the first picture came and when the picture first reached, and then kept, the
/// height and 80 % of the bitrate the camera goes out at for that many people (with
/// FOGATA_TEST_CAMERA_PEOPLE, as for that many). The moment the camera was turned on goes
/// to the file cam-start in FOGATA_CALL, which every copy reads (so all of them must share
/// that folder and the clock).
Future<void> _cameraStart(
  WidgetTester tester,
  AppState state,
  int seconds,
) async {
  final call = state.callState.call!;
  final people = int.tryParse(_env['FOGATA_TEST_CAMERA_PEOPLE'] ?? '');
  if (people != null) call.peopleForCamera = people;
  final step = cameraStep(people ?? _copies);
  // The made-up camera gives 1280x720.
  final lines = step.lines ?? 720;
  final mark = File('$_dir/cam-start');
  String num1(double v) => v.isNaN ? '?' : v.toStringAsFixed(1);
  double d(Object? v) => (v as num? ?? double.nan).toDouble();
  await _wait(tester, const Duration(seconds: 20));
  await _quality(state, 'before the camera');
  if (_role == 'a') {
    final on = DateTime.now();
    await state.setCamera(true);
    if (!call.camera) throw StateError('no camera: ${call.problem}');
    File('${mark.path}.part')
      ..writeAsStringSync('${on.microsecondsSinceEpoch} ${call.me}')
      ..renameSync(mark.path);
    _log('camera on (took ${DateTime.now().difference(on).inMilliseconds} ms)');
    final end = on.add(Duration(seconds: seconds));
    while (DateTime.now().isBefore(end)) {
      await _wait(tester, const Duration(seconds: 1));
      final t = DateTime.now().difference(on).inMilliseconds / 1000;
      final parts = <String>[];
      for (final peer in call.peers.values.toList()) {
        final v = await _watchStats(state, peer.id);
        parts.add(
          '${peer.id.substring(0, 8)} '
          '${v['camera.frameWidth']}x${v['camera.frameHeight']}, target '
          '${num1(d(v['camera.targetBitrate']) / 1000)} kbit/s, limited by '
          '${v['camera.qualityLimitationReason']} '
          '(${v['camera.qualityLimitationResolutionChanges']} changes), '
          'can send ${num1(d(v['path.availableOutgoingBitrate']) / 1000)}',
        );
      }
      _log('camera start out at ${num1(t)} s: ${parts.join('; ')}');
    }
    await state.setCamera(false);
    return;
  }
  await _until(tester, 'camera on', mark.existsSync, seconds: 120);
  final [micros, id] = mark.readAsStringSync().split(' ');
  final on = DateTime.fromMicrosecondsSinceEpoch(int.parse(micros));
  double? first, sized, fast, sizeKept, rateKept;
  var lost = 0.0, received = 0.0;
  final first0 = await _watchStats(state, id);
  var before = first0;
  var at = DateTime.now();
  final end = on.add(Duration(seconds: seconds));
  while (DateTime.now().isBefore(end)) {
    await _wait(tester, const Duration(seconds: 1));
    final now = await _watchStats(state, id);
    final took = DateTime.now().difference(at).inMilliseconds / 1000;
    at = DateTime.now();
    final t = at.difference(on).inMilliseconds / 1000;
    if (now['connection'] != before['connection']) before = {};
    double delta(String key) => d(now[key]) - d(before[key]);
    final frames = delta('camera.framesDecoded') / took;
    final kbps = delta('camera.bytesReceived') * 8 / took / 1000;
    final height = d(now['camera.frameHeight']);
    final lostNow = delta('camera.packetsLost');
    final got = delta('camera.packetsReceived');
    if (!lostNow.isNaN) lost += lostNow;
    if (!got.isNaN) received += got;
    if (first == null && d(now['camera.framesDecoded']) > 0) first = t;
    final big = height >= lines;
    final full = kbps >= step.kbps * 0.8;
    if (big) {
      sized ??= t;
      sizeKept ??= t;
    } else {
      sizeKept = null;
    }
    if (full) {
      fast ??= t;
      rateKept ??= t;
    } else {
      rateKept = null;
    }
    _log(
      'camera start in at ${num1(t)} s: '
      '${now['camera.frameWidth']}x${now['camera.frameHeight']} at '
      '${num1(frames)} fps, ${num1(kbps)} kbit/s, '
      'lost ${num1(lostNow)} of ${num1(lostNow + got)}, '
      'freezes ${now['camera.freezeCount']} '
      '(${num1(d(now['camera.totalFreezesDuration']))} s)',
    );
    before = now;
  }
  String when(double? t) => t == null ? 'never' : '${num1(t)} s';
  _log(
    'camera start summary: ${people ?? _copies} people, aiming at $lines lines '
    'and ${step.kbps} kbit/s; first picture ${when(first)}; $lines lines '
    'from ${when(sized)} (kept from ${when(sizeKept)}); '
    '${(step.kbps * 0.8).round()} kbit/s from ${when(fast)} '
    '(kept from ${when(rateKept)}); lost ${lost.round()} of '
    '${(lost + received).round()} packets; freezes '
    '${num1(d(before['camera.freezeCount']) - d(first0['camera.freezeCount']))} '
    '(${num1(d(before['camera.totalFreezesDuration']) - d(first0['camera.totalFreezesDuration']))} s)',
  );
}

/// [sdp] with only H.264 in its video sections: VP8 and its retransmission codec left out.
String _onlyH264(String sdp) {
  final lines = sdp.split('\r\n');
  final out = <String>[];
  var video = false;
  final dropped = <String>{};
  for (final line in lines) {
    if (line.startsWith('m=')) {
      video = line.startsWith('m=video ');
      dropped.clear();
    }
    final type = RegExp(r'^a=(?:rtpmap|fmtp|rtcp-fb):(\d+)')
        .firstMatch(line)
        ?.group(1);
    final name = RegExp(r'^a=rtpmap:\d+ ([^/]+)/').firstMatch(line)?.group(1);
    if (video && type != null) {
      if (name != null && name.toUpperCase() != 'H264') dropped.add(type);
      if (line.startsWith('a=fmtp:$type apt=')) {
        final apt = line.substring('a=fmtp:$type apt='.length);
        if (dropped.contains(apt)) dropped.add(type);
      }
      if (dropped.contains(type)) continue;
    }
    out.add(line);
  }
  // The m= lines name only the types left.
  return [
    for (final line in out)
      if (line.startsWith('m=video '))
        [
          ...line.split(' ').take(3),
          ...line
              .split(' ')
              .skip(3)
              .where((t) => out.any((l) => l.startsWith('a=rtpmap:$t '))),
        ].join(' ')
      else
        line,
  ].join('\r\n');
}

/// Copy b acts as a modified copy would against copy a: it offers a connection of eight
/// channels (four pictures nobody draws), and one of the call's four channels whose
/// pictures are in H.264 only (a decoder no copy uses), each as a new connection and as
/// an update, whichever side should offer; then it sends its camera at 2560 by 1440,
/// larger than any copy sends. Copy a must drop the offers and keep its connection as it
/// was, and then keep b out of the call.
Future<void> _hostile(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final other = call.peers.values.single;
  final group = state.roomState.roomGroup!;
  if (_role == 'b') {
    final made = await createPeerConnection({'sdpSemantics': 'unified-plan'});
    for (var i = 0; i < 8; i++) {
      await made.addTransceiver(
        kind: i == 0 || i == 3
            ? RTCRtpMediaType.RTCRtpMediaTypeAudio
            : RTCRtpMediaType.RTCRtpMediaTypeVideo,
        init: RTCRtpTransceiverInit(direction: TransceiverDirection.SendRecv),
      );
    }
    final offer = await made.createOffer({});
    await made.close();
    final four = await createPeerConnection({'sdpSemantics': 'unified-plan'});
    for (var i = 0; i < 4; i++) {
      await four.addTransceiver(
        kind: i == 0 || i == 3
            ? RTCRtpMediaType.RTCRtpMediaTypeAudio
            : RTCRtpMediaType.RTCRtpMediaTypeVideo,
        init: RTCRtpTransceiverInit(direction: TransceiverDirection.SendRecv),
      );
    }
    final h264 = _onlyH264((await four.createOffer({})).sdp!);
    await four.close();
    for (final sdp in [offer.sdp, h264]) {
      for (final update in [false, true]) {
        await api.sendSignal(
          groupId: group,
          deviceId: other.id,
          payload: jsonEncode({
            't': 'offer',
            'sdp': sdp,
            'update': update,
            'n': 1,
          }),
        );
      }
    }
    _log(
      'sent offers of eight channels and of pictures in H.264 only (this side '
      '${call.me.compareTo(other.id) < 0 ? 'offers' : 'does not offer'})',
    );
    await _say(state, 'offered');
    await _hear(tester, state, ['offers dropped']);
    final stream = await navigator.mediaDevices.getUserMedia({
      'audio': false,
      'video': {'width': 2560, 'height': 1440, 'frameRate': 30},
    });
    final track = stream.getVideoTracks().first;
    final channels = await other.connection!.getTransceivers();
    await channels.firstWhere((t) => t.mid == '1').sender.replaceTrack(track);
    _log('sends its camera at 2560x1440');
    await _hear(tester, state, ['kept out']);
    await track.stop();
    await stream.dispose();
    return;
  }
  final decoders = await getRtpReceiverCapabilities('video');
  _log(
    'video decoders: '
    '${{for (final codec in decoders.codecs ?? const []) codec.mimeType}.join(', ')}',
  );
  await _hear(tester, state, ['offered']);
  // The offers came before the word that says so; a moment for them to be handled.
  await _wait(tester, const Duration(seconds: 5));
  final channels = await other.connection!.getTransceivers();
  if (!other.connected || channels.length > 4) {
    throw TestFailure(
      'an offer of eight channels was taken: ${channels.length} channels, '
      '${other.connected ? 'connected' : 'not connected'}',
    );
  }
  _log('offers dropped; still connected, ${channels.length} channels');
  await _say(state, 'offers dropped');
  await _until(tester, 'b kept out', () => other.keptOut, seconds: 120);
  _log('kept b out of the call');
  // The notice it shows, in both themes.
  await _wait(tester, const Duration(milliseconds: 800));
  await _shot(tester, 'kept-out-dark');
  await state.setTheme(ThemeMode.light);
  await _wait(tester, const Duration(milliseconds: 800));
  await _shot(tester, 'kept-out-light');
  await state.setTheme(ThemeMode.dark);
  await _say(state, 'kept out');
}

/// Copy b acts as a modified copy would against copy a: its camera sends, once a second,
/// a picture far larger than any copy sends in place of an ordinary 1280 by 720 one (the
/// made-up camera does, with FOGATA_TEST_CAMERA_GIANT on copy b). Each such picture costs
/// the receiver's decoder hundreds of MB, and the statistics read every 2 s almost never
/// show one: copy a must keep b out of the call within FOGATA_TEST_HOSTILE_WAIT seconds
/// (default 20) of the first.
Future<void> _hostileGiant(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final other = call.peers.values.single;
  if (_role == 'b') {
    final stream = await navigator.mediaDevices.getUserMedia({
      'audio': false,
      'video': {'width': 1280, 'height': 720, 'frameRate': 30},
    });
    final track = stream.getVideoTracks().first;
    final channels = await other.connection!.getTransceivers();
    await channels.firstWhere((t) => t.mid == '1').sender.replaceTrack(track);
    _log(
      'sends its camera, with a picture of '
      '${_env['FOGATA_TEST_CAMERA_GIANT']} every second',
    );
    await _say(state, 'giant pictures');
    var heard = _said(state, ['kept out', 'not kept out']);
    while (heard.isEmpty) {
      await _wait(tester, const Duration(seconds: 2));
      if (other.connection != null) {
        _log(
          'camera out: ${await _cameraStats(other.connection!, 'outbound-rtp')}',
        );
      }
      heard = _said(state, ['kept out', 'not kept out']);
    }
    _log('a: ${heard.values.single}');
    await track.stop();
    await stream.dispose();
    return;
  }
  await _hear(tester, state, ['giant pictures']);
  final wait = int.tryParse(_env['FOGATA_TEST_HOSTILE_WAIT'] ?? '') ?? 20;
  final since = DateTime.now();
  // What arrives on the camera's channel, every 2 s until b is kept out.
  final connection = other.connection!;
  var reading = true;
  () async {
    while (reading && !other.keptOut) {
      _log('camera in: ${await _cameraStats(connection, 'inbound-rtp')}');
      await Future<void>.delayed(const Duration(seconds: 2));
    }
  }();
  try {
    await _until(tester, 'b kept out', () => other.keptOut, seconds: wait);
  } on StateError {
    reading = false;
    _log('b not kept out after $wait s');
    await _say(state, 'not kept out');
    throw TestFailure('pictures larger than any copy sends were not noticed');
  }
  reading = false;
  _log(
    'kept b out after ${DateTime.now().difference(since).inMilliseconds} ms',
  );
  await _say(state, 'kept out');
}

/// Copy b stops answering copy a's connectivity checks (its bridge drops the answers while
/// the file FOGATA_TEST_UNANSWERED names exists) and keeps sending, as a modified copy
/// could; once a's connection says `disconnected`, b sends its camera. Copy a logs, every
/// second, the connection's state and what it decodes on the camera's channel. With
/// FOGATA_TEST_CAMERA_GIANT on b, the camera sends a picture far larger than any copy
/// sends, first of all: a must keep b out within FOGATA_TEST_HOSTILE_WAIT seconds
/// (default 20) of it, disconnected or not.
Future<void> _hostileUnanswered(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final other = call.peers.values.single;
  if (_role == 'b') {
    final file = _env['FOGATA_TEST_UNANSWERED'];
    if (file == null) throw TestFailure('FOGATA_TEST_UNANSWERED is not set');
    File(file).writeAsStringSync('');
    _log('no answers to connectivity checks from now');
    await _say(state, 'unanswered');
    await _hear(tester, state, ['disconnected', 'still connected']);
    final stream = await navigator.mediaDevices.getUserMedia({
      'audio': false,
      'video': {'width': 1280, 'height': 720, 'frameRate': 30},
    });
    final track = stream.getVideoTracks().first;
    final channels = await other.connection!.getTransceivers();
    await channels.firstWhere((t) => t.mid == '1').sender.replaceTrack(track);
    final giant = _env['FOGATA_TEST_CAMERA_GIANT'];
    _log(
      'sends its camera${giant == null ? '' : ', with a picture of $giant every second'}',
    );
    await _say(state, giant == null ? 'camera' : 'giant camera');
    var heard = _said(state, ['measured', 'kept out', 'not kept out']);
    while (heard.isEmpty) {
      await _wait(tester, const Duration(seconds: 2));
      if (other.connection != null) {
        _log(
          'camera out: ${await _cameraStats(other.connection!, 'outbound-rtp')}',
        );
      }
      heard = _said(state, ['measured', 'kept out', 'not kept out']);
    }
    _log('a: ${heard.values.single}');
    await track.stop();
    await stream.dispose();
    File(file).deleteSync();
    return;
  }
  await _hear(tester, state, ['unanswered']);
  final since = DateTime.now();
  try {
    await _until(
      tester,
      'connection disconnected',
      () =>
          other.state ==
          RTCPeerConnectionState.RTCPeerConnectionStateDisconnected,
      seconds: 90,
    );
  } on StateError {
    _log('connection ${other.state} after 90 s');
    await _say(state, 'still connected');
    throw TestFailure('the connection did not say disconnected');
  }
  _log(
    'disconnected after ${DateTime.now().difference(since).inMilliseconds} ms '
    'without answers',
  );
  await _say(state, 'disconnected');
  final giant =
      (await _hear(tester, state, ['camera', 'giant camera'])).values.single ==
      'giant camera';
  final connection = other.connection!;
  final wait = int.tryParse(_env['FOGATA_TEST_HOSTILE_WAIT'] ?? '') ?? 20;
  final start = DateTime.now();
  // What arrives on the camera's channel, and in what state, every second.
  while (DateTime.now().difference(start).inSeconds < wait && !other.keptOut) {
    _log(
      '${other.state}; camera in: ${await _cameraStats(connection, 'inbound-rtp')}',
    );
    await _wait(tester, const Duration(seconds: 1));
  }
  if (!giant) {
    await _say(state, 'measured');
    return;
  }
  if (!other.keptOut) {
    _log('b not kept out after $wait s');
    await _say(state, 'not kept out');
    throw TestFailure('pictures larger than any copy sends were not noticed');
  }
  _log(
    'kept b out after ${DateTime.now().difference(start).inMilliseconds} ms, '
    'the connection ${other.state}',
  );
  await _say(state, 'kept out');
}

/// The camera channel's statistics of one direction ([type]), as words for the log.
Future<String> _cameraStats(RTCPeerConnection connection, String type) async {
  try {
    for (final report in await connection.getStats()) {
      final v = report.values;
      if (report.type != type || v['kind'] != 'video' || '${v['mid']}' != '1') {
        continue;
      }
      return [
        for (final key in [
          'frameWidth',
          'frameHeight',
          'framesDecoded',
          'keyFramesDecoded',
          'totalDecodeTime',
          'framesEncoded',
          'keyFramesEncoded',
          'totalEncodeTime',
          'qualityLimitationReason',
          'bytesReceived',
          'bytesSent',
        ])
          if (v[key] != null) '$key ${v[key]}',
      ].join(', ');
    }
  } catch (e) {
    return '$e';
  }
  return 'none';
}

/// Copy b acts as a modified copy would against copy a: when it is the side that offers, it
/// offers a new connection eight times a second (as fast as signals are taken) for
/// FOGATA_TEST_STORM seconds (default 20), each from a connection it closes at once. Copy
/// a logs how many connections it made meanwhile, and how long its connection with b then
/// takes to come up again; its media bridge's log (RUST_LOG=fogata_media=debug) tells the
/// TURN requests it dropped.
Future<void> _hostileOffers(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final other = call.peers.values.single;
  final group = state.roomState.roomGroup!;
  const words = ['storm over', 'no storm'];
  if (_role == 'b') {
    if (call.me.compareTo(other.id) > 0) {
      _log('b does not offer: no storm');
      await _say(state, 'no storm');
      await _hear(tester, state, ['storm seen']);
      return;
    }
    final storm = int.tryParse(_env['FOGATA_TEST_STORM'] ?? '') ?? 20;
    final end = DateTime.now().add(Duration(seconds: storm));
    var sent = 0;
    while (DateTime.now().isBefore(end)) {
      final made = await createPeerConnection({'sdpSemantics': 'unified-plan'});
      for (var i = 0; i < 4; i++) {
        await made.addTransceiver(
          kind: i == 0 || i == 3
              ? RTCRtpMediaType.RTCRtpMediaTypeAudio
              : RTCRtpMediaType.RTCRtpMediaTypeVideo,
          init: RTCRtpTransceiverInit(direction: TransceiverDirection.SendRecv),
        );
      }
      final offer = await made.createOffer({});
      await made.close();
      await api.sendSignal(
        groupId: group,
        deviceId: other.id,
        payload: jsonEncode({'t': 'offer', 'sdp': offer.sdp, 'n': ++sent}),
      );
      await _wait(tester, const Duration(milliseconds: 125));
    }
    _log('sent $sent offers of a new connection in $storm s');
    await _say(state, 'storm over');
    await _hear(tester, state, ['storm seen']);
    return;
  }
  final made = <RTCPeerConnection>{};
  var watching = true;
  () async {
    while (watching) {
      final connection = other.connection;
      if (connection != null) made.add(connection);
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
  }();
  final said = await _hear(tester, state, words, seconds: 300);
  watching = false;
  if (said.values.single == 'no storm') {
    _log('no storm: b does not offer');
    await _say(state, 'storm seen');
    return;
  }
  _log('made ${made.length} connections with b during the storm');
  final since = DateTime.now();
  await _until(tester, 'connected to b again', () => other.connected);
  _log(
    'connected to b again ${DateTime.now().difference(since).inMilliseconds} ms '
    'after the storm',
  );
  await _say(state, 'storm seen');
}

/// Copy b's screens stop (its Dart thread busy) for FOGATA_TEST_STUCK seconds (default 20)
/// while copy a turns its camera on and off four times a second, ending with it on: far
/// more call signals than b's core keeps for its screens come meanwhile. Once b's screens
/// read again they hear the signals were lost, make the connection with a anew, and must
/// be connected to a with its camera shown within a minute.
Future<void> _hostileStuck(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final other = call.peers.values.single;
  final stuck = int.tryParse(_env['FOGATA_TEST_STUCK'] ?? '') ?? 20;
  if (_role == 'b') {
    await _say(state, 'stuck');
    await _hear(tester, state, ['toggling']);
    _log('screens stopped for $stuck s');
    final until = DateTime.now().add(Duration(seconds: stuck));
    while (DateTime.now().isBefore(until)) {}
    final since = DateTime.now();
    await _until(tester, 'connected to a again with its camera', () {
      final peer = call.peers[other.id];
      return peer != null && peer.connected && peer.cam;
    }, seconds: 60);
    _log(
      'made anew: ${!identical(call.peers[other.id], other)}, '
      'back ${DateTime.now().difference(since).inMilliseconds} ms after the screens did',
    );
    await _say(state, 'recovered');
    return;
  }
  await _hear(tester, state, ['stuck']);
  await _say(state, 'toggling');
  final end = DateTime.now().add(Duration(seconds: stuck + 2));
  var toggles = 0;
  while (DateTime.now().isBefore(end)) {
    await state.setCamera(!call.camera);
    toggles++;
    await _wait(tester, const Duration(milliseconds: 250));
  }
  if (!call.camera) await state.setCamera(true);
  _log('turned the camera on or off $toggles times');
  await _hear(tester, state, ['recovered'], seconds: 120);
}

/// A number for JSON: NaN and infinities (a count missing for a moment) as null.
num? _json(double v) => v.isFinite ? double.parse(v.toStringAsFixed(2)) : null;

/// One connection's statistics for [_measureCall]: each channel's values by way
/// (`in.voice.packetsLost`, `out.camera.framesEncoded`…) and the chosen path's (`path.`).
Future<Map<String, Object?>> _measureStats(CallPeer peer) async {
  final connection = peer.connection;
  if (connection == null) return const {};
  final List<StatsReport> reports;
  try {
    reports = await connection.getStats();
  } catch (_) {
    return const {};
  }
  final values = <String, Object?>{'connection': identityHashCode(connection)};
  final selected = reports
      .where((r) => r.type == 'transport')
      .map((r) => r.values['selectedCandidatePairId'])
      .whereType<String>()
      .firstOrNull;
  for (final report in reports) {
    final v = report.values;
    final channel = switch ('${v['mid']}') {
      '0' => 'voice',
      '1' => 'camera',
      '2' => 'screen',
      _ => null,
    };
    final prefix = switch (report.type) {
      'inbound-rtp' when channel != null => 'in.$channel.',
      'outbound-rtp' when channel != null => 'out.$channel.',
      'candidate-pair'
          when report.id == selected ||
              (selected == null && v['nominated'] == true) =>
        'path.',
      _ => null,
    };
    if (prefix == null) continue;
    for (final MapEntry(:key, :value) in v.entries) {
      values['$prefix$key'] = value;
    }
  }
  return values;
}

/// Measures the call as it is for [seconds]. Every copy sends its voice (the tone, with
/// FOGATA_TEST_MIC=1), with FOGATA_TEST_CAMERA=1 also its camera, and the copy with
/// FOGATA_TEST_SCREEN shares its screen while all the others watch it. Once every picture
/// arrives and 10 s more have passed, every 2 s each copy appends one line of JSON to
/// `measure-<role>.jsonl` in FOGATA_CALL: the processor time it used (percent of one core)
/// and its resident memory, and for each other copy the round trip, the voice's packets
/// received and lost, its jitter and how much of it was concealed, and for each picture
/// that arrives its frames, freezes (as WebRTC counts them) and stalls (2 s without a
/// frame from someone who sends one), and for each picture sent its encoder, key frames
/// and key frame requests. `tools/call-lab/sum.py` turns the lines into tables.
Future<void> _measureCall(
  WidgetTester tester,
  AppState state,
  int seconds,
) async {
  final call = state.callState.call!;
  // The camera's step for this many people instead of those in the call.
  final people = int.tryParse(_env['FOGATA_TEST_CAMERA_PEOPLE'] ?? '');
  if (people != null) call.peopleForCamera = people;
  if (_camera) {
    await state.setCamera(true);
    if (!call.camera) throw StateError('no camera: ${call.problem}');
  }
  if (_share != null) {
    final sources = await call.screens();
    await call.shareScreen(
      sources.firstWhere((s) => s.type == SourceType.Screen),
    );
    if (!call.sharing) throw StateError('not sharing: ${call.problem}');
  }
  await _say(state, _share != null ? 'measure screen' : 'measure');
  final said = await _hear(tester, state, [
    'measure screen',
    'measure',
  ], seconds: 300);
  final sharer = said.entries
      .where((e) => e.value == 'measure screen')
      .map((e) => e.key)
      .firstOrNull;
  if (sharer != null) {
    final peer = _peer(state, sharer);
    call.watch(sharer.id);
    await _until(
      tester,
      "${sharer.name}'s screen shown",
      () => peer.sharing && peer.screen.videoWidth > 0,
      seconds: 180,
    );
  }
  if (_camera) {
    await _until(
      tester,
      'every camera shown',
      () => call.peers.values.every((p) => p.cam && p.camera.videoWidth > 0),
      seconds: 180,
    );
  }
  await _say(state, 'measuring');
  await _hear(tester, state, ['measuring'], seconds: 300);
  // From here the test draws no frames of its own (as _wait does, 20 a second): only those
  // the app asks for, as when someone is in a call, so the processor time is the app's.
  await Future<void>.delayed(const Duration(seconds: 10));

  double d(Object? v) => (v as num? ?? double.nan).toDouble();
  final out = File('$_dir/measure-$_role.jsonl');
  final before = <String, Map<String, Object?>>{};
  for (final peer in call.peers.values.toList()) {
    before[peer.id] = await _measureStats(peer);
  }
  var cpu = _processorSeconds();
  final threads = _threadTicks();
  var at = DateTime.now();
  final start = at;
  final processor = <double>[], memory = <double>[];
  var stalls = 0, freezes = 0.0, lost = 0.0, received = 0.0;
  while (DateTime.now().difference(start) < Duration(seconds: seconds)) {
    await Future<void>.delayed(const Duration(seconds: 2));
    final now = <String, Map<String, Object?>>{};
    for (final peer in call.peers.values.toList()) {
      now[peer.id] = await _measureStats(peer);
    }
    final took = DateTime.now().difference(at).inMilliseconds / 1000;
    at = DateTime.now();
    final cpuNow = _processorSeconds();
    final share = (cpuNow - cpu) / took * 100;
    cpu = cpuNow;
    final rss = ProcessInfo.currentRss / (1 << 20);
    if (!share.isNaN) processor.add(share);
    memory.add(rss);
    final peers = <String, Object?>{};
    for (final MapEntry(key: id, value: v) in now.entries) {
      var last = before[id] ?? const <String, Object?>{};
      // Counts of another connection than the last reading's do not compare.
      if (last['connection'] != v['connection']) last = const {};
      double delta(String key) => d(v[key]) - d(last[key]);
      final peer = call.peers[id];
      final record = <String, Object?>{
        'rtt': _json(d(v['path.currentRoundTripTime']) * 1000),
        'voiceIn': _json(delta('in.voice.packetsReceived')),
        'voiceLost': _json(delta('in.voice.packetsLost')),
        'jitter': _json(d(v['in.voice.jitter']) * 1000),
        'concealed': _json(delta('in.voice.concealedSamples')),
        'samples': _json(delta('in.voice.totalSamplesReceived')),
        'voiceOut': _json(delta('out.voice.packetsSent')),
      };
      for (final (channel, sends) in [
        ('camera', peer?.cam ?? false),
        ('screen', sharer?.id == id && (peer?.sharing ?? false)),
      ]) {
        final frames = delta('in.$channel.framesDecoded');
        final froze = delta('in.$channel.freezeCount');
        record['${channel}Sends'] = sends;
        record['${channel}Fps'] = _json(frames / took);
        record['${channel}Freezes'] = _json(froze);
        record['${channel}Frozen'] = _json(
          delta('in.$channel.totalFreezesDuration'),
        );
        record['${channel}Height'] = v['in.$channel.frameHeight'];
        record['${channel}Lost'] = _json(delta('in.$channel.packetsLost'));
        record['${channel}In'] = _json(delta('in.$channel.packetsReceived'));
        record['${channel}OutFps'] = _json(
          delta('out.$channel.framesEncoded') / took,
        );
        record['${channel}OutKbps'] = _json(
          delta('out.$channel.bytesSent') * 8 / took / 1000,
        );
        record['${channel}OutHeight'] = v['out.$channel.frameHeight'];
        record['${channel}Limit'] = v['out.$channel.qualityLimitationReason'];
        // Which encoder sends it ("libvpx (shared)": one for every connection that sends
        // the screen at that size), the key frames it encoded and the requests for one
        // that came back (PLI), to see what one watcher's losses cost the others.
        record['${channel}Encoder'] = v['out.$channel.encoderImplementation'];
        record['${channel}OutKeys'] = _json(
          delta('out.$channel.keyFramesEncoded'),
        );
        record['${channel}OutPli'] = _json(delta('out.$channel.pliCount'));
        final stalled = sends && !(frames > 0);
        record['${channel}Stall'] = stalled;
        if (stalled) stalls++;
        if (!froze.isNaN) freezes += froze;
      }
      final voiceLost = delta('in.voice.packetsLost');
      final voiceIn = delta('in.voice.packetsReceived');
      if (!voiceLost.isNaN && !voiceIn.isNaN) {
        lost += voiceLost;
        received += voiceIn;
      }
      peers[id.substring(0, 8)] = record;
    }
    before
      ..clear()
      ..addAll(now);
    out.writeAsStringSync(
      '${jsonEncode({'t': _json(at.difference(start).inMilliseconds / 1000), 'role': _role, 'copies': _copies, 'took': _json(took), 'cpu': _json(share), 'rss': _json(rss), 'peers': peers})}\n',
      mode: FileMode.append,
    );
  }
  // Where the processor time went: each kind of thread's share of one core, the most
  // first.
  final spent = DateTime.now().difference(start).inMilliseconds / 1000;
  final busy = [
    for (final MapEntry(:key, :value) in _threadTicks().entries)
      (key, (value - (threads[key] ?? 0)) / spent),
  ]..sort((a, b) => b.$2.compareTo(a.$2));
  File('$_dir/measure-$_role-threads.json').writeAsStringSync(
    jsonEncode({for (final (name, share) in busy.take(25)) name: _json(share)}),
  );
  final p = _spread(processor), m = _spread(memory);
  _log(
    'measure summary: process median ${p.median.toStringAsFixed(1)} % of a core '
    '(high ${p.high.toStringAsFixed(1)}), memory median '
    '${m.median.toStringAsFixed(0)} MB; voice lost $lost of ${lost + received}; '
    'freezes $freezes; stalls $stalls',
  );
  if (_share != null) await call.stopSharing();
  if (_camera) await state.setCamera(false);
}

/// The processor time of this process's threads so far, in clock ticks of 1/100 s, by
/// name with digits left out ("worker_thread", "llvmpipe-#"); empty where it cannot be read.
Map<String, int> _threadTicks() {
  final ticks = <String, int>{};
  if (!Platform.isLinux) return ticks;
  try {
    for (final task in Directory('/proc/self/task').listSync()) {
      try {
        final stat = File('${task.path}/stat').readAsStringSync();
        final name = stat
            .substring(stat.indexOf('(') + 1, stat.lastIndexOf(')'))
            .replaceAll(RegExp('[0-9]+'), '#');
        final fields = stat.substring(stat.lastIndexOf(')') + 2).split(' ');
        ticks[name] =
            (ticks[name] ?? 0) + int.parse(fields[11]) + int.parse(fields[12]);
      } catch (_) {
        // A thread that ended meanwhile.
      }
    }
  } catch (_) {}
  return ticks;
}

/// Notes in FOGATA_CALL when this copy did something to its microphone, for whatever runs
/// the copies to compare with what the sound server saw it record: a file
/// `mark-<role>-<event>` holding the time in milliseconds since the epoch.
void _mark(String event) {
  File('$_dir/mark-$_role-$event')
      .writeAsStringSync('${DateTime.now().millisecondsSinceEpoch}\n');
  _log('mark $event');
}

/// Waits for a file that whatever runs the copies writes to FOGATA_CALL.
Future<void> _rig(WidgetTester tester, String name, {int seconds = 120}) =>
    _until(
      tester,
      'rig: $name',
      () => File('$_dir/$name').existsSync(),
      seconds: seconds,
    );

/// One check of the microphone, run with tools/call-lab/mic.sh, which watches what each
/// copy records from the sound server and plays the part of the person who unplugs it:
///
/// - muted: copy b enters muted, stays so 20 s, unmutes for 10 s and mutes again for
///   15 s (mic.sh: b records nothing until it unmutes, and nothing a moment after it
///   mutes); copy a hears it only while unmuted.
/// - switch: copy a changes its input to the silent one and back while b listens (mic.sh:
///   a's recording follows).
/// - unplug: copy a's microphone goes away in the middle of the call and comes back
///   (mic.sh unloads and loads it); b must hear a again once it is back. What a shows
///   meanwhile is logged.
/// - broken, alsa, hang: see [_noSoundServer].
/// - quit: see [_quitWhileOpening]; copy b only waits for a to go.
/// - devices: see [_listedAfterCall]; copy b only stays in the call.
/// - rejoin: see [_rejoin].
/// - echo: see [_echo].
/// - noise: see [_noise].
/// - restart: see [_soundRestart].
Future<void> _micCase(WidgetTester tester, AppState state, String name) async {
  final call = state.callState.call!;
  switch (name) {
    case 'muted':
      if (_role == 'b') {
        _mark('joined-muted');
        if (call.mic) throw StateError('entered with the microphone on');
        await _wait(tester, const Duration(seconds: 20));
        _mark('unmute');
        await state.setMic(true);
        await _wait(tester, const Duration(seconds: 10));
        _mark('mute');
        await state.setMic(false);
        await _wait(tester, const Duration(seconds: 15));
        _mark('end');
        await _say(state, 'mic done');
      } else {
        await _hear(tester, state, ['mic done'], seconds: 180);
      }
    case 'late' || 'alone':
      // Copy a's microphone opens only after b connected (FOGATA_TEST_MIC_LATE), or a was
      // alone in the call before b came (alone): b hears it without anyone muting and
      // unmuting, which agrees on the connection again and opens the microphone anew.
      if (_role == 'a') {
        await _hear(tester, state, ['heard'], seconds: 120);
      } else {
        final _Other other = (id: call.peers.keys.single, name: _names['a']!);
        final peer = _peer(state, other);
        await _until(
          tester,
          'a heard speaking',
          () => peer.speaking,
          seconds: 40,
        );
        if (name == 'alone') {
          // And keeps coming as a voice, not as the silence Opus sends now and then
          // (a few hundred bytes in 4 s): a's microphone track disabled sent only that.
          final before = await _voiceBytesFrom(peer);
          await _wait(tester, const Duration(seconds: 4));
          final bytes = await _voiceBytesFrom(peer) - before;
          _log('voice from a: $bytes bytes in 4 s');
          if (bytes < 4000) {
            throw StateError('only $bytes bytes of voice from a in 4 s');
          }
        }
        await _say(state, 'heard');
      }
    case 'switch':
      await _voiceSteps(tester, state, unplug: false);
    case 'unplug':
      await _voiceSteps(tester, state, unplug: true);
    case 'denied':
      // Windows with the microphone refused to applications (Settings › Privacy ›
      // Microphone, or no input at all; tools/call-lab/win-mic.ps1): copy a enters with it
      // on; within 25 s the call must say it cannot use it and show it off, and keep
      // running. Copy b waits.
      if (_role == 'a') {
        // Said as a toast (AppState.micUnusable), with the microphone off in the call.
        bool told() => state.toasts.list.any(
          (t) =>
              t.text == l10n.callMicSilent('windows') ||
              t.text == l10n.callNoMicrophone,
        );
        await _until(
          tester,
          'the microphone told unusable',
          () => told() && !call.mic,
          seconds: 25,
        );
        var sent = 0, samples = 0.0;
        for (final peer in call.peers.values) {
          for (final r
              in await peer.connection?.getStats() ?? const <StatsReport>[]) {
            if (r.type == 'outbound-rtp' && '${r.values['mid']}' == '0') {
              sent += (r.values['packetsSent'] as num? ?? 0).toInt();
            }
            if (r.type == 'media-source' && r.values['kind'] == 'audio') {
              samples += (r.values['totalSamplesDuration'] as num? ?? 0)
                  .toDouble();
            }
          }
        }
        _log(
          'denied microphone: mic ${call.mic}, problem ${call.problem}, '
          'told ${state.toasts.list.map((t) => t.text).toList()}, '
          'voice packets sent $sent, seconds of sound read $samples',
        );
        await _say(state, 'mic done');
      } else {
        await _hear(tester, state, ['mic done'], seconds: 180);
      }
    case 'quit':
      // Copy a quits on its own while entering; nothing more to do here.
      break;
    case 'devices':
      if (_role == 'a') await _listedAfterCall(tester, state);
    case 'media':
      await _mediaLetGo(tester, state);
    case 'rejoin':
      await _rejoin(tester, state);
    case 'echo':
      await _echo(tester, state);
    case 'noise':
      await _noise(tester, state);
    case 'restart':
      await _soundRestart(tester, state);
    default:
      throw StateError('no microphone check called $name');
  }
}

/// What the video library holds that a camera or a shared screen adds: local streams and
/// tracks, cameras' capturers, and what stops a screen's capture. The microphone's own
/// stream and track stay while the call does.
const _mediaHeld = ['streams', 'tracks', 'capturers', 'stoppers'];

/// tools/call-lab/mic.sh media: copy a turns its camera on (the made-up one,
/// FOGATA_TEST_CAMERA=1) and shares the screen while b shows both, then stops both: the
/// video library must hold no more local streams, tracks, capturers or screen stoppers
/// than before, while the call goes on (a camera or a screen still captured would show
/// there), and nothing at all on either copy once the call ended. Copy b also lets go of
/// the streams it made to show a's pictures, which hold tracks it received.
Future<void> _mediaLetGo(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final problems = <String>[];
  void compare(String when, Map<String, int> before, Map<String, int> now) {
    _log('held $when: $now (before: $before)');
    for (final key in _mediaHeld) {
      if ((now[key] ?? 0) > (before[key] ?? 0)) {
        problems.add('${now[key]} $key held $when, ${before[key] ?? 0} before');
      }
    }
  }

  await _until(
    tester,
    'connected',
    () => call.peers.values.any((p) => p.connected),
    seconds: 120,
  );
  if (_role == 'a') {
    final before = await _held();
    for (var round = 1; round <= 2; round++) {
      await state.setCamera(true);
      if (!call.camera) throw StateError('no camera: ${call.problem}');
      final sources = await call.screens();
      await call.shareScreen(
        sources.firstWhere(
          (s) => s.type == SourceType.Screen,
          orElse: () => throw StateError('no screen to share'),
        ),
      );
      if (!call.sharing) throw StateError('not sharing: ${call.problem}');
      _log('round $round: camera and screen on, held ${await _held()}');
      await _say(state, 'media on $round');
      await _hear(tester, state, ['media seen $round']);
      await state.setCamera(false);
      await call.stopSharing();
      await _wait(tester, const Duration(seconds: 3));
      compare(
        'after camera and screen stopped ($round)',
        before,
        await _held(),
      );
      await _say(state, 'media off $round');
      await _hear(tester, state, ['media let go $round']);
    }
  } else {
    final a = (await _hear(tester, state, ['media on 1'])).keys.single;
    final peer = _peer(state, a);
    for (var round = 1; round <= 2; round++) {
      if (round > 1) await _hearFrom(tester, state, a, 'media on $round');
      await _until(
        tester,
        "a's camera shown ($round)",
        () => peer.cam && peer.camera.videoWidth > 0,
        seconds: 60,
      );
      await _until(
        tester,
        "a's screen offered ($round)",
        () => peer.sharing,
        seconds: 60,
      );
      call.watch(a.id);
      await _until(
        tester,
        "a's screen shown ($round)",
        () => peer.screen.videoWidth > 0,
        seconds: 60,
      );
      _log('round $round: shown, held ${await _held()}');
      await _say(state, 'media seen $round');
      await _hearFrom(tester, state, a, 'media off $round');
      await _until(
        tester,
        "a's camera and screen gone ($round)",
        () => !peer.cam && !peer.sharing,
        seconds: 30,
      );
      call.watch(null);
      await _wait(tester, const Duration(seconds: 2));
      await _say(state, 'media let go $round');
    }
  }
  await _say(state, 'media done');
  await _hear(tester, state, ['media done']);
  await state.endCallForTests();
  await _wait(tester, const Duration(seconds: 5));
  final after = await _held();
  _log('held 5 s after the call: $after');
  for (final e in after.entries) {
    if (e.value > 0) problems.add('${e.value} ${e.key} held after the call');
  }
  if (problems.isNotEmpty) {
    throw StateError('media not let go of: ${problems.join('; ')}');
  }
}

/// A call back within this long after its room is entered again counts as back soon. It
/// normally takes 1-4 s (the others hear the device is back, its call starts, one offer
/// and its answer, the first packets); 6 s leaves a margin for a slow start or network,
/// and still catches a first offer lost on the way, after which the call came back only
/// at the next try, 5 s or more later.
const _rejoinWithin = Duration(seconds: 6);

/// Packets of voice that count as voice flowing again: a fifth of a second of it.
const _flowPackets = 10;

/// The packets of voice received so far on [peer]'s connection (mid 0); 0 without one, or
/// when its statistics do not come within [Call.statsWithin]: one asked as the connection
/// closes (the other left) may never come, and the step would wait for it for ever. Each
/// such reading is logged.
Future<int> _voicePacketsFrom(CallPeer? peer) async {
  final connection = peer?.connection;
  if (connection == null) return 0;
  final List<StatsReport> reports;
  try {
    reports = await connection.getStats().timeout(Call.statsWithin);
  } on TimeoutException {
    _log('statistics of ${peer!.id.substring(0, 8)} not read in time');
    return 0;
  } catch (e) {
    // The connection closed as it was read.
    _log('statistics of ${peer!.id.substring(0, 8)}: $e');
    return 0;
  }
  var packets = 0;
  for (final r in reports) {
    if (r.type == 'inbound-rtp' && '${r.values['mid']}' == '0') {
      packets += (r.values['packetsReceived'] as num? ?? 0).toInt();
    }
  }
  return packets;
}

/// What [other] said in the room's chat after `test: [prefix] `, once it has.
Future<String> _heardWith(
  WidgetTester tester,
  AppState state,
  _Other other,
  String prefix,
) async {
  String? said() {
    for (final UiChatLine line
        in state.roomState.room?.chat ?? const <UiChatLine>[]) {
      if (line.sender == other.id && line.text.startsWith('test: $prefix ')) {
        return line.text.substring('test: $prefix '.length);
      }
    }
    return null;
  }

  await _until(
    tester,
    'heard from ${other.name}: $prefix',
    () => said() != null,
    seconds: 180,
  );
  return said()!;
}

/// tools/call-lab/mic.sh rejoin, or two copies on two computers (FOGATA_TEST_MIC_CASE=rejoin,
/// both with the microphone on): copy b leaves the voice room and enters it again,
/// FOGATA_TEST_REJOIN_TIMES times (5 when left out) at once, as many after a pause of 1 s and
/// as many after one of 5 s (the call came back late depending on the moment); then copy a
/// does the same, so that the device with the smaller id and the one with the larger leave,
/// whichever each copy got. Each time both measure, from the moment the one leaving entered
/// again, until voice from the other flows to them again ([_flowPackets] more packets than
/// before, on the call's connection there is then); the one that stayed counts from the
/// moment the other says it entered, by the other's clock (on two computers, as close as
/// their clocks are). Each time is logged, the copies tell each other theirs, and each logs
/// the least, the median and the most, for each direction and each that left. A time over
/// [_rejoinWithin] fails the copy that measured it, once all are done.
Future<void> _rejoin(WidgetTester tester, AppState state) async {
  if (_copies != 2) throw StateError('the rejoin check takes two copies');
  final call = state.callState.call!;
  if (!call.mic) {
    throw StateError('the rejoin check needs the microphone on in both copies');
  }
  final times = int.tryParse(_env['FOGATA_TEST_REJOIN_TIMES'] ?? '') ?? 5;
  final group = call.group;
  final _Other other = (
    id: call.peers.keys.single,
    name: _names[_role == 'a' ? 'b' : 'a']!,
  );
  final measured = <Map<String, Object>>[];
  Future<void> poll() => _wait(tester, const Duration(milliseconds: 100));
  for (final leaving in ['b', 'a']) {
    for (final pause in [0, 1, 5]) {
      for (var round = 1; round <= times; round++) {
        final tag = '$leaving${pause}s$round';
        final int ms;
        final String direction;
        if (_role == leaving) {
          direction = 'to';
          await _say(state, 'rejoin $tag leave');
          await _hearFrom(tester, state, other, 'rejoin $tag ready');
          await state.leaveRoom();
          await _wait(tester, Duration(seconds: pause));
          final entered = DateTime.now();
          await state.openRoom(group);
          await _say(
            state,
            'rejoin $tag entered ${entered.millisecondsSinceEpoch}',
          );
          final end = entered.add(const Duration(seconds: 30));
          while (await _voicePacketsFrom(
                state.callState.call?.peers[other.id],
              ) <
              _flowPackets) {
            if (DateTime.now().isAfter(end)) {
              await _say(state, 'rejoin $tag took never');
              throw StateError(
                'rejoin $tag: no voice from ${other.name} in 30 s',
              );
            }
            await poll();
          }
          ms = DateTime.now().difference(entered).inMilliseconds;
        } else {
          direction = 'from';
          await _hearFrom(tester, state, other, 'rejoin $tag leave');
          // What it is connected with now, before it may go: its connection here, and the
          // call of its own it names in its signals.
          CallPeer? peer() => state.callState.call?.peers[other.id];
          final was = (
            peer: peer(),
            connection: peer()?.connection,
            call: peer()?.theirCall,
          );
          bool same() =>
              peer() == was.peer &&
              peer()?.connection == was.connection &&
              peer()?.theirCall == was.call;
          var count = await _voicePacketsFrom(was.peer);
          await _say(state, 'rejoin $tag ready');
          // It goes, and maybe comes again at once: what it is connected with changes (a
          // new connection, or a new call of its own), or nothing more comes on it.
          var still = DateTime.now();
          final end = DateTime.now().add(const Duration(seconds: 60));
          while (same()) {
            await poll();
            final n = await _voicePacketsFrom(peer());
            if (n != count) {
              count = n;
              still = DateTime.now();
            } else if (DateTime.now().difference(still) >
                const Duration(milliseconds: 600)) {
              break;
            }
            if (DateTime.now().isAfter(end)) {
              throw StateError('rejoin $tag: ${other.name} never left');
            }
          }
          // And its voice comes again: counted on the connection there is now, from
          // nothing on one made since.
          final stopped = same() ? count : 0;
          final stoppedOn = peer()?.connection;
          while (true) {
            final now = peer();
            final n = await _voicePacketsFrom(now);
            final from = now?.connection == stoppedOn ? stopped : 0;
            if (n - from >= _flowPackets) break;
            if (DateTime.now().isAfter(end)) {
              await _say(state, 'rejoin $tag took never');
              throw StateError(
                'rejoin $tag: no voice from ${other.name} again',
              );
            }
            await poll();
          }
          final flowing = DateTime.now().millisecondsSinceEpoch;
          final entered = int.parse(
            await _heardWith(tester, state, other, 'rejoin $tag entered'),
          );
          ms = flowing - entered;
        }
        _log(
          'rejoin $tag: $leaving left, pause $pause s, voice $direction the one '
          'that left after $ms ms',
        );
        measured.add({'l': leaving, 'p': pause, 'd': direction, 'ms': ms});
        await _say(state, 'rejoin $tag took $ms');
        final theirs = await _heardWith(
          tester,
          state,
          other,
          'rejoin $tag took',
        );
        if (theirs == 'never') {
          throw StateError('rejoin $tag: ${other.name} measured none');
        }
        // A moment in the call before the next round.
        await _wait(tester, const Duration(seconds: 1));
      }
    }
  }
  await _say(state, 'rejoin times ${jsonEncode(measured)}');
  final theirs = (jsonDecode(
    await _heardWith(tester, state, other, 'rejoin times'),
  ) as List).cast<Map<String, dynamic>>();
  final all = [...measured, ...theirs];
  for (final leaving in ['b', 'a']) {
    final parts = <String>[];
    for (final direction in ['to', 'from']) {
      final ms = [
        for (final m in all)
          if (m['l'] == leaving && m['d'] == direction) m['ms'] as int,
      ]..sort();
      if (ms.isEmpty) continue;
      parts.add(
        'voice $direction the one that left: min ${ms.first} median '
        '${ms[ms.length ~/ 2]} max ${ms.last} ms (${ms.length})',
      );
    }
    _log('rejoin summary: $leaving left, ${parts.join('; ')}');
  }
  final late = [
    for (final m in measured)
      if ((m['ms'] as int) > _rejoinWithin.inMilliseconds)
        '${m['l']} left, pause ${m['p']} s: ${m['ms']} ms',
  ];
  if (late.isNotEmpty) {
    throw StateError(
      'voice back later than ${_rejoinWithin.inSeconds} s after entering again: '
      '${late.join(', ')}',
    );
  }
}

/// tools/call-lab/mic.sh echo (two copies, both with the microphone on, each its own
/// tone): the sequence in which a person on Linux heard their own voice come back in a
/// shared screen's sound. Copy a shares its screen with the other applications' sound
/// (the lab plays a tone of its own as another application), and copy b watches it,
/// leaves the room, enters it again and only then watches it again, once for each pause
/// of FOGATA_TEST_ECHO_PAUSES ("1 10" when left out: the seconds out of the room). Each
/// copy plays what it hears into a silent output of its own, which mic.sh records for
/// every stretch b marks (`mark-b-<stretch>-start` and `-end`):
///
/// - before: b watches, before leaving;
/// - `gone<n>`: b out of the room for the n-th time, with a alone in the call (its own level
///   read through a loop of its own meanwhile), the pause less 2 s, when it is 6 s or more;
/// - `between<n>`: b back in the call, not watching yet;
/// - `after<n>`: b watching again.
///
/// Logs every 200 ms, in `level-<role>.txt` of FOGATA_CALL, this copy's own voice level as
/// its processing puts it out (the voice channel's media source, before the codec), with
/// the time in milliseconds since the epoch, until the timer is cancelled.
Timer _logLevels(AppState state) {
  final levels = File('$_dir/level-$_role.txt');
  return Timer.periodic(const Duration(milliseconds: 200), (_) async {
    final connection =
        state.callState.call?.peers.values.firstOrNull?.connection;
    if (connection == null) return;
    try {
      final reports = await connection.getStats();
      final byId = {for (final r in reports) r.id: r};
      for (final r in reports) {
        if (r.type != 'outbound-rtp' || '${r.values['mid']}' != '0') continue;
        final level = byId[r.values['mediaSourceId']]?.values['audioLevel'];
        if (level is num) {
          levels.writeAsStringSync(
            '${DateTime.now().millisecondsSinceEpoch} $level\n',
            mode: FileMode.append,
          );
        }
      }
    } catch (_) {}
  });
}

/// tools/call-lab/mic.sh noise. Copy a entered with noise suppression on and its
/// microphone on, a steady noise in it (a fan's, from mic.sh), and never muted. It logs
/// its own level as its processing puts it out ([_logLevels]) and marks four stretches of
/// FOGATA_TEST_NOISE_SECONDS (6 when left out), `mark-a-<stretch>-start` and `-end`:
///
/// - `start`: as it entered, b connected;
/// - `off`: noise suppression turned off;
/// - `on`: turned on again;
/// - `again`: after muting and unmuting.
///
/// mic_check.py judges them: the noise must be as low in `start` as in `on` and `again`,
/// well under `off`. Copy b only stays in the call.
Future<void> _noise(WidgetTester tester, AppState state) async {
  if (_copies != 2) throw StateError('the noise check takes two copies');
  if (_role != 'a') {
    await _hear(tester, state, ['noise done'], seconds: 300);
    return;
  }
  final call = state.callState.call!;
  if (!call.mic) throw StateError('the noise check needs the microphone on');
  if (!call.noiseSuppression) {
    throw StateError('entered with noise suppression off');
  }
  final each = Duration(
    seconds: int.tryParse(_env['FOGATA_TEST_NOISE_SECONDS'] ?? '') ?? 6,
  );
  Future<void> stretch(String name) async {
    _mark('$name-start');
    await _wait(tester, each);
    _mark('$name-end');
  }

  final logging = _logLevels(state);
  try {
    await stretch('start');
    await state.setNoiseSuppression(false);
    await stretch('off');
    await state.setNoiseSuppression(true);
    await stretch('on');
    await state.setMic(false);
    await _wait(tester, const Duration(seconds: 1));
    await state.setMic(true);
    await _wait(tester, const Duration(seconds: 1));
    await stretch('again');
  } finally {
    logging.cancel();
  }
  await _say(state, 'noise done');
}

/// mic_check.py judges them: neither copy may play its own pitch, and b must play a's
/// voice throughout and the screen's sound only while it watches.
Future<void> _echo(WidgetTester tester, AppState state) async {
  if (_copies != 2) throw StateError('the echo check takes two copies');
  final first = state.callState.call!;
  if (!first.mic) {
    throw StateError('the echo check needs the microphone on in both copies');
  }
  final group = first.group;
  final _Other other = (
    id: first.peers.keys.single,
    name: _names[_role == 'a' ? 'b' : 'a']!,
  );
  // For tools/call-lab/mic_check.py's steady tones: Opus's silence suppression may drop a
  // steady tone on its way to the other copy.
  final logging = _logLevels(state);
  try {
    await _echoSteps(tester, state, first, group, other);
  } finally {
    logging.cancel();
  }
}

Future<void> _echoSteps(
  WidgetTester tester,
  AppState state,
  Call first,
  String group,
  _Other other,
) async {
  if (_role == 'a') {
    final sources = await first.screens();
    await first.shareScreen(
      sources.firstWhere(
        (s) => s.type == SourceType.Screen,
        orElse: () => throw StateError('no screen to share'),
      ),
    );
    if (!first.sharing) throw StateError('not sharing: ${first.problem}');
    _log(
      'sharing; with sound: ${first.sharingSound}, '
      'can share sound: ${first.canShareSound}',
    );
    if (!first.sharingSound) {
      throw StateError("the screen is shared without the applications' sound");
    }
    await _say(state, 'echo shared');
    await _hear(tester, state, ['echo done'], seconds: 900);
    return;
  }
  final pauses = (_env['FOGATA_TEST_ECHO_PAUSES'] ?? '1 10')
      .split(' ')
      .where((p) => p.isNotEmpty)
      .map(int.parse)
      .toList();
  // Long enough for a whole period of the copies' turns to be measured after the first
  // 3 s (tools/call-lab/tone.py: 16 s, with each copy's talk and steady tone in it).
  Future<void> stretch(String name, int seconds) async {
    _mark('$name-start');
    await _wait(tester, Duration(seconds: seconds));
    _mark('$name-end');
  }

  // Watches a's screen in the call there is now, and waits for its sound to arrive.
  Future<void> watch(String when) async {
    final call = state.callState.call!;
    final peer = _peer(state, other);
    await _until(
      tester,
      "a's screen with sound offered $when",
      () => peer.sharing && peer.sound,
      seconds: 60,
    );
    call.watch(other.id);
    await _until(
      tester,
      "a's screen shown $when",
      () => peer.screen.videoWidth > 0,
      seconds: 60,
    );
    final end = DateTime.now().add(const Duration(seconds: 30));
    var packets = 0;
    while ((packets = await _screenSoundIn(peer)) < 100) {
      if (DateTime.now().isAfter(end)) {
        throw StateError("no sound with a's screen $when: $packets packets");
      }
      await _wait(tester, const Duration(milliseconds: 300));
    }
    _log("a's screen sound $when: $packets packets");
    // The sound settles (the jitter buffer, the gain) before it is judged.
    await _wait(tester, const Duration(seconds: 3));
  }

  await _hearFrom(tester, state, other, 'echo shared');
  await watch('before leaving');
  await stretch('before', 24);
  for (var round = 1; round <= pauses.length; round++) {
    final pause = pauses[round - 1];
    await state.leaveRoom();
    if (pause >= 6) {
      await _wait(tester, const Duration(seconds: 1));
      await stretch('gone$round', pause - 2);
      await _wait(tester, const Duration(seconds: 1));
    } else {
      await _wait(tester, Duration(seconds: pause));
    }
    await state.openRoom(group);
    await _until(
      tester,
      'back in the call with a ($round)',
      () =>
          state.callState.call?.peers[other.id]?.connected == true &&
          state.callState.call?.watching == null,
      seconds: 90,
    );
    await _wait(tester, const Duration(seconds: 3));
    await stretch('between$round', 20);
    await watch('after entering again ($round)');
    await stretch('after$round', 24);
  }
  await _say(state, 'echo done');
}

/// The bytes of voice received so far from [peer] (its voice channel, mid 0).
Future<int> _voiceBytesFrom(CallPeer peer) async {
  var bytes = 0;
  for (final r in await peer.connection?.getStats() ?? const <StatsReport>[]) {
    if (r.type == 'inbound-rtp' && '${r.values['mid']}' == '0') {
      bytes += (r.values['bytesReceived'] as num? ?? 0).toInt();
    }
  }
  return bytes;
}

/// How long after the lab made its devices again (the sound server back) each copy must
/// list its own microphone and speaker, record, and hear the other again
/// ([_soundRestart]). The voice engine tries the server again every 5 s at most, the
/// plugin's input watcher too, and the lab's devices are made a moment after the server
/// answers.
const _restartWithin = Duration(seconds: 20);

/// tools/call-lab/mic.sh restart: both copies talk (their tones) and hear each other; the
/// lab then stops the session's own sound server and starts it again as
/// FOGATA_TEST_RESTART says (pulse: PipeWire's PulseAudio service alone; all: the three
/// daemons; down20: the three, 20 s away; gone: the three, a minute away), and makes its
/// devices again. Meanwhile each copy must stay in the call with its microphone on (not
/// turned off as reading nothing), and with `gone` say by the minute's end that the
/// sound system stopped. From the moment the lab's devices are back (`devices-back`),
/// each must list its own microphone and speaker again, record (its own level), and hear
/// the other again (more of its voice received, and its level), within
/// [_restartWithin]; a server away over 5 s is told gone and back (one back within a
/// moment, pulse or all, need not be).
Future<void> _soundRestart(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  final how = _env['FOGATA_TEST_RESTART'] ?? 'pulse';
  final role = _role == 'a' ? 'b' : 'a';
  final _Other other = (id: call.peers.keys.single, name: _names[role]!);
  final peer = _peer(state, other);
  await _until(
    tester,
    'heard ${other.name}',
    () => peer.speaking,
    seconds: 120,
  );
  _mark('restart-ready');
  await _rig(tester, 'sound-stopped', seconds: 300);
  // What it said, as toasts go after a few seconds: looked at every time round.
  final said = <String>{};
  void look() => said.addAll(state.toasts.list.map((t) => t.text));
  bool told(String text) {
    look();
    return said.contains(text);
  }

  final suffix = _env['FOGATA_TEST_DEVICES'] ?? '';
  // This copy's own microphone and speaker listed, and how many sound devices are.
  Future<({bool mine, int count})> listed() async {
    final devices = await navigator.mediaDevices.enumerateDevices();
    final sound = devices.where(
      (d) => d.kind == 'audioinput' || d.kind == 'audiooutput',
    );
    final mine =
        sound.any((d) => d.label.contains('FogataMic$suffix')) &&
        sound.any((d) => d.label.contains('FogataTest$suffix'));
    return (mine: mine, count: sound.length);
  }

  final stopped = DateTime.now();
  var sawNone = false;
  var checkedGone = false;
  while (!File('$_dir/devices-back').existsSync()) {
    if (DateTime.now().difference(stopped) > const Duration(seconds: 200)) {
      throw StateError('the lab did not bring the sound server back');
    }
    if (state.callState.call != call) {
      throw StateError('the call ended while the sound server was away');
    }
    look();
    final now = await listed();
    if (now.count == 0 && !sawNone) {
      sawNone = true;
      _log('restart: no sound device listed');
    }
    if (File('$_dir/still-gone').existsSync() && !checkedGone) {
      checkedGone = true;
      look();
      _log(
        'restart: a minute without a sound server: mic ${call.mic}, told '
        '$said',
      );
      if (!told(l10n.callSoundSystemLost)) {
        throw StateError('not told the sound system stopped');
      }
      if (!call.mic) throw StateError('the microphone was turned off');
    }
    await _wait(tester, const Duration(milliseconds: 500));
  }
  final back = DateTime.now();
  final before = await _voiceBytesFrom(peer);
  int? listedAfter, recordingAfter, heardAfter;
  bool done() =>
      listedAfter != null && recordingAfter != null && heardAfter != null;
  while (!done() && DateTime.now().difference(back) < _restartWithin) {
    look();
    final ms = DateTime.now().difference(back).inMilliseconds;
    if (listedAfter == null && (await listed()).mine) listedAfter = ms;
    if (recordingAfter == null && call.speaking) recordingAfter = ms;
    if (heardAfter == null &&
        peer.speaking &&
        await _voiceBytesFrom(peer) > before) {
      heardAfter = ms;
    }
    await _wait(tester, const Duration(milliseconds: 250));
  }
  look();
  _log(
    'restart $how: no device listed meanwhile: $sawNone; own devices '
    'listed again after ${listedAfter ?? 'never'} ms, recording after '
    '${recordingAfter ?? 'never'} ms, ${other.name} heard after '
    '${heardAfter ?? 'never'} ms; mic ${call.mic}; told $said',
  );
  if (!call.mic) throw StateError('the microphone was turned off');
  if (listedAfter == null) throw StateError('its devices not listed again');
  if (recordingAfter == null) throw StateError('not recording again');
  if (heardAfter == null) throw StateError('${other.name} not heard again');
  // Away over 5 s, it is told; back sooner (pulse, all: unless the voice engine's
  // next try came later), it need not be. Told gone, it is told back.
  final away = how == 'down20' || how == 'gone';
  if (away && !told(l10n.callSoundSystemLost)) {
    throw StateError('not told the sound system stopped');
  }
  if (told(l10n.callSoundSystemLost)) {
    await _until(
      tester,
      'told: ${l10n.callSoundSystemBack}',
      () => told(l10n.callSoundSystemBack),
      seconds: 10,
    );
  }
  // The call stays a while, so that whatever watches the sound server sees it record.
  await _wait(tester, const Duration(seconds: 5));
  await _say(state, 'restarted');
  await _hear(tester, state, ['restarted'], seconds: 120);
}

/// Copy a talks (the tone) while b listens; a then changes to the silent input and back
/// (switch), or its microphone goes away and comes back (unplug).
Future<void> _voiceSteps(
  WidgetTester tester,
  AppState state, {
  required bool unplug,
}) async {
  final call = state.callState.call!;
  if (_role == 'a') {
    await _hear(tester, state, ['heard'], seconds: 120);
    if (unplug) {
      _mark('unplug-ready');
      await _rig(tester, 'unplugged');
      _log('devices after unplugging: ${await _inputs()}');
      // The person is told, naming the input used now (the session's default).
      final gone = l10n.callMicGone('LabDefaultMic');
      bool shown(String text) => state.toasts.list.any((t) => t.text == text);
      await _until(tester, 'told: $gone', () => shown(gone), seconds: 10);
      await _wait(tester, const Duration(seconds: 8));
      _log('while unplugged: mic ${call.mic}, problem ${call.problem}');
      _mark('replug-ready');
      await _rig(tester, 'replugged');
      _log('devices after plugging in: ${await _inputs()}');
      await _until(
        tester,
        'told: ${l10n.callMicBack}',
        () => shown(l10n.callMicBack),
        seconds: 10,
      );
    } else {
      _mark('to-quiet');
      await state.setAudioInput(_quiet);
      await _say(state, 'quiet');
      await _hear(tester, state, ['saw quiet']);
      _mark('to-tone');
      await state.setAudioInput(_tone);
    }
    await _say(state, 'back');
    await _hear(tester, state, ['heard again'], seconds: 120);
    _log('at the end: mic ${call.mic}, problem ${call.problem}');
    return;
  }
  // Two copies: the other one is a.
  final _Other other = (id: call.peers.keys.single, name: _names['a']!);
  final peer = _peer(state, other);
  await _until(tester, 'a heard speaking', () => peer.speaking, seconds: 60);
  await _say(state, 'heard');
  if (!unplug) {
    await _hearFrom(tester, state, other, 'quiet');
    await _until(
      tester,
      'a silent on the quiet input',
      () => peer.mic && !peer.speaking,
      seconds: 15,
    );
    // And stays so a while: the silent input is the one in use.
    await _wait(tester, const Duration(seconds: 3));
    if (peer.speaking) throw StateError('a heard on the quiet input');
    await _say(state, 'saw quiet');
  } else {
    // Whatever a records while its microphone is gone is the system's default input: the
    // log says whether it was heard.
    final end = DateTime.now().add(const Duration(seconds: 60));
    var silentFor = 0;
    while (!File('$_dir/replugged').existsSync() &&
        DateTime.now().isBefore(end)) {
      await _wait(tester, const Duration(milliseconds: 500));
      if (File('$_dir/unplugged').existsSync() && !peer.speaking) silentFor++;
    }
    _log('a silent for ${silentFor / 2} s while unplugged');
  }
  await _hearFrom(tester, state, other, 'back');
  await _until(tester, 'a heard again', () => peer.speaking, seconds: 30);
  await _say(state, 'heard again');
}

/// Copy a (FOGATA_TEST_MIC=broken) has no working sound server, where WebRTC's voice
/// engine would end the whole application. With [check] `broken` there is none at all; with
/// `alsa` none either while the computer's sound devices are there (as stand-ins); with
/// `hang` one takes the connection and never answers, which must be asked twice before it
/// counts as none (tools/call-lab/mic_check.py checks the two answers in a's log). In every
/// case no call may be made, and the application must say why, keep running and keep the
/// room. Copy b only waits for a's result.
Future<void> _noSoundServer(
  WidgetTester tester,
  AppState state,
  String check,
) async {
  if (_role != 'a') {
    // a tells it has no call: b stops trying to connect and shows a without sound.
    await _until(
      tester,
      'a shown without sound',
      () =>
          state.callState.call?.peers.values.any(
            (p) => p.voiceless && p.connection == null,
          ) ??
          false,
      seconds: 120,
    );
    await _until(
      tester,
      "a's result",
      () => File('$_dir/a-result.txt').existsSync(),
      seconds: 300,
    );
    return;
  }
  bool told() => state.toasts.list.any((t) => t.text == l10n.callNoSoundSystem);
  final asked = DateTime.now();
  await _until(
    tester,
    'no sound system told',
    told,
    seconds: check == 'hang' ? 90 : 20,
  );
  _log(
    'no sound server: told after '
    '${DateTime.now().difference(asked).inMilliseconds} ms',
  );
  // Still running a while later, still in the room, with no call.
  await _wait(tester, const Duration(seconds: 10));
  if (state.callState.call != null) throw StateError('a call without sound');
  if (state.roomState.roomGroup == null) throw StateError('the room was left');
}

/// The sound inputs listed now, by name.
Future<String> _inputs() async => [
  for (final d in await navigator.mediaDevices.enumerateDevices())
    if (d.kind == 'audioinput') d.label,
].join(' | ');

/// What the checks of the devices listed found wrong, said at the end ([_listedAfterCall]).
final _unlisted = <String>[];

/// Lists the sound devices as the settings do, and notes ([when]) whether this copy's own
/// microphone and speaker (FOGATA_TEST_DEVICES) are among them, or with [micGone] its
/// speaker and not its microphone. Tried again for up to [within]: a device plugged in or
/// out is listed a moment later.
Future<void> _checkListed(
  WidgetTester tester,
  String when, {
  bool micGone = false,
  Duration within = Duration.zero,
}) async {
  final suffix = _env['FOGATA_TEST_DEVICES'] ?? '';
  final end = DateTime.now().add(within);
  while (true) {
    final inputs = <String>[], outputs = <String>[];
    Object? error;
    try {
      for (final d in await navigator.mediaDevices.enumerateDevices()) {
        if (d.kind == 'audioinput') inputs.add(d.label);
        if (d.kind == 'audiooutput') outputs.add(d.label);
      }
    } catch (e) {
      error = e;
    }
    final mic = inputs.any((l) => l.contains('FogataMic$suffix'));
    final speaker = outputs.any((l) => l.contains('FogataTest$suffix'));
    final ok = speaker && mic != micGone;
    if (ok || !DateTime.now().isBefore(end)) {
      _log(
        'devices $when: ${ok ? 'as expected' : 'WRONG'}; '
        '${inputs.length} inputs $inputs, ${outputs.length} outputs $outputs'
        '${error == null ? '' : ', error $error'}',
      );
      if (!ok) {
        _unlisted.add(
          '$when: ${inputs.length} inputs, ${outputs.length} outputs listed',
        );
      }
      return;
    }
    await _wait(tester, const Duration(milliseconds: 250));
  }
}

/// Runs the settings' microphone test as its button does, on this copy's own devices,
/// until it hears the tone (5 s at most), and stops it.
Future<void> _micTestOnce(WidgetTester tester, AppState state) async {
  await micTest.start(
    input: state.prefs.audioInput,
    output: state.prefs.audioOutput,
  );
  final end = DateTime.now().add(const Duration(seconds: 5));
  while (micTest.level.value <= 0 && DateTime.now().isBefore(end)) {
    await _wait(tester, const Duration(milliseconds: 100));
  }
  final heard = micTest.level.value > 0 && !micTest.failed;
  await micTest.stop();
  if (!heard) _unlisted.add('a microphone test heard nothing');
}

/// The lists before a microphone test, right after it and a few seconds later.
Future<void> _listedAroundTest(
  WidgetTester tester,
  AppState state,
  String when,
) async {
  await _checkListed(tester, '$when, before the microphone test');
  await _micTestOnce(tester, state);
  await _checkListed(tester, '$when, right after the microphone test');
  await _wait(tester, const Duration(seconds: 5));
  await _checkListed(tester, '$when, 5 s after the microphone test');
}

/// glibc's `struct mallinfo2`: what malloc holds, across all its arenas.
final class _MallInfo extends ffi.Struct {
  @ffi.Size()
  external int arena;
  @ffi.Size()
  external int ordblks;
  @ffi.Size()
  external int smblks;
  @ffi.Size()
  external int hblks;
  @ffi.Size()
  external int hblkhd;
  @ffi.Size()
  external int usmblks;
  @ffi.Size()
  external int fsmblks;
  @ffi.Size()
  external int uordblks;
  @ffi.Size()
  external int fordblks;
  @ffi.Size()
  external int keepcost;
}

/// One line of what this process holds (Linux): resident memory, its anonymous part,
/// malloc's bytes in use, free in its arenas and mapped on their own, threads and open
/// files. With [trim], malloc gives its free memory back first.
String _memoryNow({bool trim = false}) {
  int field(String file, String name) {
    try {
      final line = File(file)
          .readAsLinesSync()
          .firstWhere((l) => l.startsWith('$name:'), orElse: () => '');
      return int.tryParse(line.replaceAll(RegExp('[^0-9]'), '')) ?? 0;
    } catch (_) {
      return 0;
    }
  }

  var malloc = '';
  try {
    final libc = ffi.DynamicLibrary.process();
    if (trim) {
      libc.lookupFunction<ffi.Int Function(ffi.Size), int Function(int)>(
        'malloc_trim',
      )(0);
    }
    final info = libc
        .lookupFunction<_MallInfo Function(), _MallInfo Function()>(
          'mallinfo2',
        )();
    malloc =
        ', malloc in use ${info.uordblks ~/ 1024} kB, free '
        '${info.fordblks ~/ 1024} kB, mapped ${info.hblkhd ~/ 1024} kB';
  } catch (_) {}
  var files = 0;
  try {
    files = Directory('/proc/self/fd').listSync().length;
  } catch (_) {}
  return 'rss ${field('/proc/self/status', 'VmRSS')} kB, anonymous '
      '${field('/proc/self/smaps_rollup', 'Anonymous')} kB$malloc, '
      'threads ${field('/proc/self/status', 'Threads')}, files $files';
}

/// Connections left on purpose by [_soakTests] (FOGATA_TEST_LEAK), to measure what a leak
/// costs.
final _leaked = <RTCPeerConnection>[];

/// [count] more microphone tests, what the process holds logged every 20; with
/// FOGATA_TEST_LEAK=open each test also leaves an idle connection open, with `closed` one
/// closed but never freed (the leak the fifth audit fixed), to measure what that costs.
Future<void> _soakTests(WidgetTester tester, AppState state, int count) async {
  _log('soak 0 of $count tests: ${_memoryNow()}, held ${await _held()}');
  for (var i = 1; i <= count; i++) {
    await _micTestLeaking(tester, state);
    if (i % 20 == 0) {
      await _wait(tester, const Duration(seconds: 1));
      _log('soak $i of $count tests: ${_memoryNow()}, held ${await _held()}');
    }
  }
  _log('soak after malloc_trim: ${_memoryNow(trim: true)}');
}

/// One microphone test ([_micTestOnce]), with the leak FOGATA_TEST_LEAK asks for.
Future<void> _micTestLeaking(WidgetTester tester, AppState state) async {
  await _micTestOnce(tester, state);
  final leak = _env['FOGATA_TEST_LEAK'];
  if (leak == 'open' || leak == 'closed') {
    final c = await createPeerConnection({'iceServers': []});
    if (leak == 'closed') await c.close();
    _leaked.add(c);
  }
}

/// How many of each thing the video library holds (its `fogataCounts`: connections, their
/// observers, local streams and tracks, capturers, renderers...); empty where it cannot
/// tell (macOS).
Future<Map<String, int>> _held() async {
  try {
    final counts = await WebRTC.invokeMethod<Map<Object?, Object?>, dynamic>(
      'fogataCounts',
    );
    return {
      for (final e in (counts ?? const {}).entries)
        '${e.key}': (e.value as num).toInt(),
    };
  } catch (_) {
    return const {};
  }
}

/// Copy a leaves its call and comes back [count] times while b stays, each time until
/// connected to b and 3 s more; what the process holds logged every 5.
Future<void> _soakCalls(WidgetTester tester, AppState state, int count) async {
  final group = state.callState.call!.group;
  _log('calls 0 of $count: ${_memoryNow()}, held ${await _held()}');
  for (var i = 1; i <= count; i++) {
    await state.endCallForTests();
    await _wait(tester, const Duration(seconds: 1));
    await state.startCallForTests(group);
    await _until(
      tester,
      'connected again ($i)',
      () =>
          state.callState.call != null &&
          state.callState.call!.peers.isNotEmpty &&
          state.callState.call!.peers.values.every((p) => p.connected),
      seconds: 60,
    );
    await _wait(tester, const Duration(seconds: 3));
    if (i % 5 == 0) {
      _log('calls $i of $count: ${_memoryNow()}, held ${await _held()}');
    }
  }
  _log('calls after malloc_trim: ${_memoryNow(trim: true)}');
}

/// Copy a, in a call with b after a microphone test before it ([_listedAroundTest]): its
/// call ends, and the lists must still hold its devices at once and a few seconds later,
/// around another microphone test, after 20 more (after which the video library must hold
/// no more connections, streams or tracks than before), with its microphone unplugged and plugged in again (by mic.sh), and around a
/// last test. The settings list the devices this way while nothing else runs.
Future<void> _listedAfterCall(WidgetTester tester, AppState state) async {
  await _wait(tester, const Duration(seconds: 3));
  await _checkListed(tester, 'in the call');
  final calls = int.tryParse(_env['FOGATA_TEST_CALL_SOAK'] ?? '');
  if (calls != null) await _soakCalls(tester, state, calls);
  await state.endCallForTests();
  await _checkListed(tester, 'right after the call');
  await _wait(tester, const Duration(seconds: 5));
  await _checkListed(tester, '5 s after the call');
  // Nothing of the call is left (the voice engine's own idle connection is not counted).
  final afterCall = await _held();
  _log('held 5 s after the call: $afterCall');
  for (final e in afterCall.entries) {
    if (e.value > 0) _unlisted.add('${e.value} ${e.key} held after the call');
  }
  await _listedAroundTest(tester, state, 'after the call');
  // A few tests first, for what only the first ones make; then 20 counted.
  for (var i = 0; i < 3; i++) {
    await _micTestOnce(tester, state);
  }
  await _wait(tester, const Duration(seconds: 2));
  final soak = int.tryParse(_env['FOGATA_TEST_MIC_SOAK'] ?? '');
  if (soak != null) await _soakTests(tester, state, soak);
  // What is kept is counted, not measured: over 20 tests the resident memory moved by
  // 0.6 to 37 MB with nothing kept, as malloc's free space came and went (it gave 100 MB
  // back on malloc_trim), while 20 connections leaked on purpose (open, or closed and never
  // freed) did not show at all. Each test makes and frees three connections, a stream and
  // its track: one of them kept a test is 20 more at the end.
  final heldBefore = await _held();
  final memoryBefore = _memoryNow();
  for (var i = 0; i < 20; i++) {
    await _micTestLeaking(tester, state);
  }
  await _wait(tester, const Duration(seconds: 2));
  final heldAfter = await _held();
  _log(
    'held over 20 microphone tests: $heldBefore to $heldAfter; memory from '
    '$memoryBefore to ${_memoryNow()}',
  );
  for (final e in heldAfter.entries) {
    final grew = e.value - (heldBefore[e.key] ?? 0);
    if (grew > 0) _unlisted.add('$grew more ${e.key} held after 20 tests');
  }
  await _checkListed(tester, 'after 20 more microphone tests');
  _mark('unplug-ready');
  await _rig(tester, 'unplugged');
  await _checkListed(
    tester,
    'with the microphone unplugged',
    micGone: true,
    within: const Duration(seconds: 10),
  );
  _mark('replug-ready');
  await _rig(tester, 'replugged');
  await _checkListed(
    tester,
    'with the microphone plugged in again',
    within: const Duration(seconds: 10),
  );
  await _listedAroundTest(tester, state, 'at the end');
  if (state.callState.call != null) _unlisted.add('a call started again');
  if (_unlisted.isNotEmpty) {
    throw StateError('devices not as expected: ${_unlisted.join('; ')}');
  }
}

/// Copy a enters the room with its microphone on and quits the application
/// FOGATA_TEST_QUIT_AFTER milliseconds later (0 when left out), while the microphone may
/// still be opening, as the tray's Quit does. Whatever runs the copies checks that the
/// process ends by itself, in time and without a crash, and records nothing after.
Future<void> _quitWhileOpening(WidgetTester tester, AppState state) async {
  final after = int.tryParse(_env['FOGATA_TEST_QUIT_AFTER'] ?? '') ?? 0;
  _mark('entering');
  unawaited(state.openRoom(state.contacts.groups.first.id));
  if (after > 0) await Future<void>.delayed(Duration(milliseconds: after));
  _log(
    'quitting: in the call ${state.callState.call != null}, '
    'mic ${state.callState.call?.mic}',
  );
  _mark('quit');
  File('$_dir/$_role-result.txt').writeAsStringSync('quit\n');
  await quitApplication();
  // Not reached when quitting ends the process; the runner then says how long it took.
  _mark('quit-returned');
  await _wait(tester, const Duration(seconds: 30));
}
