// Voice messages between two copies of the app on one computer. Build once with
//
//   flutter build linux --debug -t integration_test/voice_test.dart
//
// and start copy a (Leo) and copy b (Marta) with FOGATA_ROLE=a or b, a FOGATA_DATA_DIR
// each and the same FOGATA_DIRECT folder, where they leave each other the friend link and
// their steps. FOGATA_TEST_MIC names each copy's microphone as the voice engine lists it
// (a virtual one fed with a known recording); each copy's sound goes to the null sink in
// PULSE_SINK. Copy b runs twice: FOGATA_PHASE=1, then, once it has quit, FOGATA_PHASE=2
// with ALSA unable to open anything (ALSA_CONFIG_PATH pointing at an empty file), where
// recording finds no microphone. FOGATA_SHOTS is a folder for pictures of the window, in
// the dark and the light theme.
//
// Each run writes <role><phase>.log and <role><phase>-result.txt ("passed" or why it
// failed) to FOGATA_DIRECT, and b1 writes b-times.txt: when each playback of Leo's
// message started and ended, by the computer's clock, to find them in what its sink
// played. a-voice.txt names the file Leo's copy keeps of what it sent.
//
// A few states cannot be made to happen on cue and are set up in the page for their
// picture only: the last minute of 15 (a real recording would take 14 minutes), a
// message from Marta that did not arrive before she went offline, and one whose
// download failed.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/api/voice.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _phase = _env['FOGATA_PHASE'] ?? '1';
final _dir = _env['FOGATA_DIRECT'] ?? Directory.systemTemp.path;

void _log(String line) => File('$_dir/$_role$_phase.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

void _step(String name) {
  File('$_dir/$_role-$name').writeAsStringSync('done');
  _log('step: $name');
}

bool _other(String name) =>
    File('$_dir/${_role == 'a' ? 'b' : 'a'}-$name').existsSync();

int _now() => DateTime.now().millisecondsSinceEpoch;

Future<void> _shot(WidgetTester tester, String name) async {
  await _wait(tester, const Duration(milliseconds: 600));
  final dir = _env['FOGATA_SHOTS'];
  if (dir == null) return;
  final view = tester.binding.renderViews.first;
  final layer = view.debugLayer! as OffsetLayer;
  final image = await layer.toImage(Offset.zero & view.size);
  final data = await image.toByteData(format: ui.ImageByteFormat.png);
  await File('$dir/$name.png').writeAsBytes(data!.buffer.asUint8List());
  _log('shot: $name');
}

/// A picture in the dark theme, then the same in the light one.
Future<void> _shots(WidgetTester tester, AppState state, String name) async {
  await _shot(tester, name);
  await state.setTheme(ThemeMode.light);
  await _shot(tester, '$name-light');
  await state.setTheme(ThemeMode.dark);
  await _wait(tester, const Duration(milliseconds: 300));
}

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
    if (DateTime.now().isAfter(end)) {
      await _shot(tester, '$_role$_phase-failed');
      throw StateError('not in time: $what');
    }
    await _wait(tester, const Duration(milliseconds: 100));
  }
  _log(what);
}

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _until(tester, 'shown: $finder', () => finder.evaluate().isNotEmpty);
  await tester.ensureVisible(finder.first);
  await _wait(tester, const Duration(milliseconds: 200));
  await tester.tap(finder.first);
  await _wait(tester, const Duration(milliseconds: 300));
}

AppState _state(WidgetTester tester) =>
    AppScope.read(tester.element(find.byType(Shell)));

Future<void> _write(WidgetTester tester, String text) async {
  final field = find.descendant(
    of: find.byKey(const ValueKey('direct-message')),
    matching: find.byType(TextField),
  );
  await _until(tester, 'the message box', () => field.evaluate().isNotEmpty);
  tester.widget<TextField>(field).controller!.text = text;
  await _wait(tester, const Duration(milliseconds: 200));
  await _tap(tester, find.byKey(const ValueKey('direct-send')));
}

/// The voice messages of the chat shown, oldest first, with their message.
List<(UiDirectItem, UiDirectFile)> _voices(AppState state) => [
  for (final i in state.direct.page?.items ?? const <UiDirectItem>[])
    for (final f in i.files)
      if (f.voice != null) (i, f),
];

/// A mouse over [finder], held there for a picture of its tooltip.
Future<TestGesture> _hover(WidgetTester tester, Finder finder) async {
  await _until(tester, 'shown: $finder', () => finder.evaluate().isNotEmpty);
  await _wait(tester, const Duration(milliseconds: 500));
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: Offset.zero);
  await mouse.moveTo(tester.getCenter(finder));
  await _wait(tester, const Duration(milliseconds: 1500));
  return mouse;
}

Future<void> _away(WidgetTester tester, TestGesture mouse) async {
  await mouse.moveTo(Offset.zero);
  await mouse.removePointer();
  await _wait(tester, const Duration(milliseconds: 400));
}

/// The chat's page with one of its files standing as [fileState], for a picture.
void _stage(AppState state, String file, UiFileState fileState) {
  final page = state.direct.page!;
  UiDirectItem item(UiDirectItem i) => UiDirectItem(
    id: i.id,
    mine: i.mine,
    atMs: i.atMs,
    text: i.text,
    files: [
      for (final f in i.files)
        f.id != file
            ? f
            : UiDirectFile(
                id: f.id,
                name: f.name,
                size: f.size,
                kind: f.kind,
                voice: f.voice,
                state: fileState,
                received: 0,
              ),
    ],
    timerMs: i.timerMs,
    delivered: i.delivered,
    expiresMs: i.expiresMs,
    pinned: i.pinned,
    pin: i.pin,
    call: i.call,
  );
  state.direct.page = UiDirectPage(
    friend: page.friend,
    online: page.online,
    away: false,
    lastSeenMs: page.lastSeenMs,
    connected: page.connected,
    items: [for (final i in page.items) item(i)],
    more: page.more,
    pinned: page.pinned,
    timerMs: page.timerMs,
    endedMs: page.endedMs,
    usage: page.usage,
    altered: page.altered,
  );
  state.direct.changed();
}

/// Pictures of a page set up with [_stage], in both themes, set up again right before each:
/// what the core says meanwhile reads the page again.
Future<void> _stagedShots(
  WidgetTester tester,
  AppState state,
  String file,
  UiFileState fileState,
  String name,
) async {
  for (final light in [false, true]) {
    if (light) await state.setTheme(ThemeMode.light);
    await _wait(tester, const Duration(milliseconds: 400));
    _stage(state, file, fileState);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    final dir = _env['FOGATA_SHOTS'];
    if (dir != null) {
      final view = tester.binding.renderViews.first;
      final layer = view.debugLayer! as OffsetLayer;
      final image = await layer.toImage(Offset.zero & view.size);
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      final shot = light ? '$name-light' : name;
      await File('$dir/$shot.png').writeAsBytes(data!.buffer.asUint8List());
      _log('shot: $shot');
    }
  }
  await state.setTheme(ThemeMode.dark);
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('voice messages between two copies', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      if (_role == 'a') {
        await _leo(tester);
      } else if (_phase == '1') {
        await _marta(tester);
      } else {
        await _martaWithoutMicrophone(tester);
      }
      File('$_dir/$_role$_phase-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$_role$_phase-result.txt')
          .writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

Future<AppState> _welcome(WidgetTester tester, String name) async {
  await app.main();
  await _until(
    tester,
    'welcome',
    () => find.byType(TextField).evaluate().isNotEmpty,
  );
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      name;
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  final mic = _env['FOGATA_TEST_MIC'];
  if (mic != null) await state.setAudioInput(mic);
  return state;
}

Future<void> _leo(WidgetTester tester) async {
  final state = await _welcome(tester, 'Leo');
  final voice = state.direct.voice;
  final link = await state.makeInvitation(code: false);
  File('$_dir/link.txt').writeAsStringSync(link!.text);
  await _until(
    tester,
    'a friend request',
    () => state.contacts.requests.incoming.isNotEmpty,
    seconds: 300,
  );
  await state.answer(state.contacts.requests.incoming.first.person.id, true);
  await _until(tester, 'friends', () => state.contacts.friends.isNotEmpty);
  final marta = state.contacts.friends.single.person;
  state.go(Screen.friends);
  await _tap(tester, find.byKey(ValueKey('message:${marta.id}')));
  await _until(tester, 'the chat', () => state.direct.page != null);
  await _until(
    tester,
    'Marta wrote',
    () =>
        state.direct.page?.items.any(
          (i) => i.text == 'Are you coming on Saturday after all?',
        ) ??
        false,
    seconds: 300,
  );
  await _write(
    tester,
    'I think so, I will tell you in a voice note, it is a long story',
  );

  // v1: the microphone button, with its tooltip.
  final mic = find.byKey(const ValueKey('voice-record'));
  var mouse = await _hover(tester, mic);
  await _shots(tester, state, 'v1-mic-button');
  await _away(tester, mouse);

  // v2: one click records; the bar shows the time and the voice coming in.
  await tester.tap(mic);
  await _until(
    tester,
    'recording',
    () => (voice.recording?.capturing ?? false),
    seconds: 10,
  );
  final started = _now();
  await _until(
    tester,
    'seven seconds',
    () => (voice.recording?.elapsedMs ?? 0) >= 7000,
    seconds: 20,
  );
  await _shots(tester, state, 'v2-recording');
  _log('recorded ${voice.recording?.elapsedMs} ms in ${_now() - started} ms');
  // Enter sends it.
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await _until(
    tester,
    'the voice message delivered',
    () => _voices(state).any((v) => v.$1.mine && v.$1.delivered),
    seconds: 120,
  );
  final mine = _voices(state).firstWhere((v) => v.$1.mine).$2;
  File('$_dir/a-voice.txt').writeAsStringSync(
    '${mine.path}\n${mine.voice!.durationMs}\n${mine.voice!.bars.join(',')}\n',
  );
  if (mine.name != 'Voice message.ogg' || mine.size > 4 << 20) {
    throw StateError('sent as ${mine.name}, ${mine.size} bytes');
  }
  final preview = state.direct.chat(marta.id);
  if (preview?.lastVoiceMs == null || !(preview?.lastMine ?? false)) {
    throw StateError('the list does not say a voice message');
  }
  _step('sent-voice');

  // Marta plays it, then answers with one of hers.
  await _until(tester, 'Marta played it', () => _other('played'), seconds: 300);
  await _until(
    tester,
    'Marta\'s voice message here',
    () =>
        _voices(state).any((v) => !v.$1.mine && v.$2.state == UiFileState.here),
    seconds: 300,
  );
  final hers = _voices(state).firstWhere((v) => !v.$1.mine).$2;
  if (hers.voice!.played) throw StateError('hers shows as played already');
  // Shown once the page built with it.
  await _until(
    tester,
    'the dot on hers',
    () => find.byKey(ValueKey('voice-new:${hers.id}')).evaluate().isNotEmpty,
    seconds: 10,
  );

  // v3: hers at 1.5×, halfway.
  await _tap(tester, find.byKey(ValueKey('voice-speed:${hers.id}')));
  await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
  await _until(
    tester,
    'halfway',
    () =>
        (voice.playback?.positionMs ?? 0) >=
        (hers.voice!.durationMs * 0.4).round(),
    seconds: 30,
  );
  await _shots(tester, state, 'v3-voice-messages');
  // Paused, it stays where it was; played again, it goes on from there. The pictures can
  // take until hers is about to end on a busy computer, and a tap meant to pause it that
  // comes after its end plays it again from its start. So the pause waits for a moment
  // with seconds of it still to play: hers goes on if it has them, else it is let end and
  // played again from its start.
  int? wallMsLeft() {
    final p = voice.playback;
    if (p == null || p.file != hers.id || !p.playing) return null;
    return ((p.durationMs - p.positionMs) / p.speed).round();
  }

  await _until(
    tester,
    'hers far from its end, or ended',
    () => (wallMsLeft() ?? 3000) >= 3000,
    seconds: 30,
  );
  if (!voice.playing(hers.id)) {
    await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
    await _until(
      tester,
      'hers played again',
      () => voice.playing(hers.id),
      seconds: 10,
    );
  }
  await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
  final pausedAt = voice.playback!.positionMs;
  await _wait(tester, const Duration(seconds: 1));
  if (voice.playback!.playing || voice.playback!.positionMs != pausedAt) {
    throw StateError('pausing did not pause');
  }
  await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
  await _wait(tester, const Duration(milliseconds: 600));
  if (voice.playback!.positionMs <= pausedAt) {
    throw StateError('it did not go on from where it was');
  }
  voice.stopPlaying();
  await state.direct.refresh();
  if (!_voices(state).firstWhere((v) => v.$2.id == hers.id).$2.voice!.played) {
    throw StateError('hers still shows as not played');
  }

  // Esc deletes a recording without asking; nothing goes.
  final before = _voices(state).length;
  await tester.tap(mic);
  await _until(
    tester,
    'recording again',
    () => voice.recording?.capturing ?? false,
    seconds: 10,
  );
  await _wait(tester, const Duration(seconds: 2));
  await tester.sendKeyEvent(LogicalKeyboardKey.escape);
  await _until(tester, 'deleted', () => voice.recordingFor == null);
  await _wait(tester, const Duration(seconds: 2));
  await state.direct.refresh();
  if (_voices(state).length != before) throw StateError('Esc sent it');

  // v2b: the last minute, set up for the picture.
  voice
    ..recordingFor = marta.id
    ..recording = UiVoiceRecording(
      elapsedMs: 852000,
      bars: Uint8List.fromList(List.generate(160, (i) => 120 + (i * 37 % 110))),
      capturing: true,
      full: false,
      failed: false,
    )
    ..changed();
  await _shots(tester, state, 'v2b-recording-last-minute');
  voice
    ..recordingFor = null
    ..recording = null
    ..changed();

  // v4a: in a call the microphone belongs to the call; playing still works.
  _step('calling');
  await state.callFriend(marta.id);
  await _until(
    tester,
    'in the call',
    () => state.callState.activeCall?.phase == UiCallPhase.connected,
    seconds: 120,
  );
  await state.direct.show(marta.id);
  await _until(tester, 'the chat again', () => state.direct.page != null);
  mouse = await _hover(tester, mic);
  await _shots(tester, state, 'v4a-in-call');
  await _away(tester, mouse);
  await tester.tap(mic);
  await _wait(tester, const Duration(seconds: 1));
  if (voice.recordingFor != null) throw StateError('it recorded in a call');
  await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
  await _until(
    tester,
    'playing in the call',
    () => voice.playing(hers.id),
    seconds: 10,
  );
  await _tap(tester, find.byKey(ValueKey('voice-play:${hers.id}')));
  await state.hangUp();
  await _until(
    tester,
    'the call over',
    () => state.callState.activeCall == null,
    seconds: 60,
  );
  _step('call-done');
  await state.direct.show(marta.id);
  await _until(tester, 'the chat once more', () => state.direct.page != null);

  // v4d: a message that could not be fetched, set up for the picture.
  voice.stopPlaying();
  state.say(
    l10n.directStateFileFailed('Marta’s connection dropped.'),
    error: true,
  );
  await _stagedShots(tester, state, hers.id, UiFileState.missing, 'v4d-failed');
  await state.direct.refresh();

  // v4b: Marta goes offline; what is here still plays.
  _step('online-done');
  await _until(
    tester,
    'Marta offline',
    () => !(state.direct.page?.online ?? true),
    seconds: 300,
  );
  await _until(
    tester,
    'the locked box',
    () => find.byKey(const ValueKey('direct-locked')).evaluate().isNotEmpty,
  );
  await _stagedShots(
    tester,
    state,
    hers.id,
    UiFileState.missing,
    'v4b-friend-offline',
  );
  await state.direct.refresh();
  await _tap(tester, find.byKey(ValueKey('voice-play:${mine.id}')));
  await _until(
    tester,
    'mine plays offline',
    () => voice.playing(mine.id),
    seconds: 10,
  );
  voice.stopPlaying();
  _step('offline-done');
  await _until(tester, 'Marta back', () => _other('back-done'), seconds: 600);
}

Future<void> _marta(WidgetTester tester) async {
  final state = await _welcome(tester, 'Marta');
  final voice = state.direct.voice;
  final file = File('$_dir/link.txt');
  await _until(
    tester,
    'link',
    () => file.existsSync() && file.readAsStringSync().isNotEmpty,
    seconds: 300,
  );
  await state.useInvitation(file.readAsStringSync());
  await _until(
    tester,
    'friends',
    () => state.contacts.friends.isNotEmpty,
    seconds: 300,
  );
  final leo = state.contacts.friends.single.person;
  state.go(Screen.friends);
  await _tap(tester, find.byKey(ValueKey('message:${leo.id}')));
  await _until(tester, 'the chat', () => state.direct.page != null);
  await _write(tester, 'Are you coming on Saturday after all?');
  // Home while Leo records: his message comes as any message does.
  state.goHome();
  await _until(
    tester,
    'Leo\'s voice message in the list',
    () => state.direct.chat(leo.id)?.lastVoiceMs != null,
    seconds: 300,
  );
  await _wait(tester, const Duration(seconds: 1));
  if (!state.played.contains(UiSound.message)) {
    throw StateError('no sound for the voice message');
  }
  if ((state.direct.chat(leo.id)?.unread ?? 0) < 1) {
    throw StateError('the voice message is not unread');
  }
  await _shot(tester, 'b-list-preview');
  await _tap(tester, find.byKey(ValueKey('direct-row:${leo.id}')));
  await _until(
    tester,
    'his voice message here by itself',
    () =>
        _voices(state).any((v) => !v.$1.mine && v.$2.state == UiFileState.here),
    seconds: 120,
  );
  final his = _voices(state).firstWhere((v) => !v.$1.mine).$2;
  await _shots(tester, state, 'v3-not-played-yet');

  // At 1×, 1.5× and 2×, each to its end; when each played goes to b-times.txt.
  final times = StringBuffer();
  for (final speed in [1.0, 1.5, 2.0]) {
    if (voice.speedOf(his.id) != speed) {
      await _tap(tester, find.byKey(ValueKey('voice-speed:${his.id}')));
    }
    await _wait(tester, const Duration(seconds: 2));
    final start = _now();
    await tester.tap(find.byKey(ValueKey('voice-play:${his.id}')));
    await _until(
      tester,
      'playing at $speed',
      () => voice.playing(his.id),
      seconds: 10,
    );
    if (speed == 1.5) await _shots(tester, state, 'v3-playing-1.5x');
    await _until(
      tester,
      'played at $speed',
      () => !voice.playing(his.id),
      seconds: 60,
    );
    final end = _now();
    final p = voice.playback!;
    times.writeln('$speed $start $end ${p.positionMs} ${p.durationMs}');
    _log('played at $speed in ${end - start} ms (${p.durationMs} ms long)');
    if (p.positionMs != p.durationMs) {
      throw StateError('it stopped at ${p.positionMs} of ${p.durationMs}');
    }
  }
  File('$_dir/b-times.txt').writeAsStringSync(times.toString());
  await state.direct.refresh();
  if (!_voices(state).firstWhere((v) => v.$2.id == his.id).$2.voice!.played) {
    throw StateError('his still shows as not played');
  }
  _step('played');

  // Her answer: ten seconds.
  await tester.tap(find.byKey(const ValueKey('voice-record')));
  await _until(
    tester,
    'recording',
    () => voice.recording?.capturing ?? false,
    seconds: 10,
  );
  await _until(
    tester,
    'ten seconds',
    () => (voice.recording?.elapsedMs ?? 0) >= 10000,
    seconds: 30,
  );
  await _tap(tester, find.byKey(const ValueKey('voice-send')));
  await _until(
    tester,
    'hers delivered',
    () => _voices(state).any((v) => v.$1.mine && v.$1.delivered),
    seconds: 120,
  );
  _step('sent-voice');

  // Leo calls: answered here.
  await _until(
    tester,
    'the call rings',
    () => state.callState.incomingCall != null,
    seconds: 300,
  );
  await state.acceptCall(state.callState.incomingCall!);
  await _until(
    tester,
    'the call over',
    () => _other('call-done'),
    seconds: 300,
  );
  await _until(tester, 'Leo done', () => _other('online-done'), seconds: 300);
  await state.direct.show(leo.id);
  await _wait(tester, const Duration(seconds: 1));
  _step('quit');
}

Future<void> _martaWithoutMicrophone(WidgetTester tester) async {
  await app.main();
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  final voice = state.direct.voice;
  await _until(
    tester,
    'the chat listed',
    () => state.direct.chats.isNotEmpty,
    seconds: 120,
  );
  final leo = state.direct.chats.first.friend;
  await _tap(tester, find.byKey(ValueKey('direct-row:${leo.id}')));
  await _until(
    tester,
    'Leo online',
    () => state.direct.page?.online ?? false,
    seconds: 300,
  );
  await _tap(tester, find.byKey(const ValueKey('voice-record')));
  await _until(tester, 'the note', () => voice.micProblem, seconds: 20);
  if (voice.recordingFor != null) throw StateError('it says it records');
  await _shots(tester, state, 'v4c-no-microphone');
  await _tap(tester, find.byKey(const ValueKey('voice-settings')));
  await _until(
    tester,
    'the voice settings',
    () => state.nav.settingsOpen,
    seconds: 10,
  );
  await _shot(tester, 'v4c-settings');
  _step('back-done');
}
