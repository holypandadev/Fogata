// The main things a person does, done by two copies of the app the way a person does them,
// on any system: the smoke test that runs before a build reaches anyone. Build once with
//
//   flutter build linux --debug -t integration_test/smoke_test.dart
//
// (or `flutter build macos` / `flutter build windows`) and start copy a (Ana) and copy b
// (Bea) with FOGATA_ROLE=a or b, a FOGATA_DATA_DIR and FOGATA_DOWNLOADS each, and the same
// FOGATA_SMOKE folder, where they leave each other the friend link and their steps. Each
// writes <role>.log and <role>-result.txt ("passed" or why it failed) there; whatever runs
// them (tools/smoke-linux.sh, tools/smoke-windows.ps1, tools/smoke-macos.sh) also tells a
// copy that died from one that failed.
//
// The two make profiles, become friends, write to each other in a direct chat, make a
// group, enter its voice room with the microphone off, turn the camera on (with
// FOGATA_TEST_CAMERA=1 on Linux, a made-up one; elsewhere only with FOGATA_SMOKE_CAMERA=1,
// for a computer whose camera may be used) and see each other's, then Ana shares her screen
// with the room's button and Bea watches it, Ana sends Bea a file, and both leave.
//
// The screen goes through the same steps as a person's click: the button, then the list of
// screens and windows (the app's own dialog, where the system gives a list: X11, Windows,
// macOS) or the system's dialog (the desktop portal under Wayland). Ana first presses the
// button twice in a row and cancels what opens, then presses it again and shares. Under
// Wayland the portal's dialog is answered by whatever stands for the person there: the
// smoke session's picker (tools/smoke-wayland/picker.py), which reads its answer from
// $SMOKE_PICKER/answer and writes what it did to $SMOKE_PICKER/dialogs.
//
// FOGATA_SMOKE_MEASURE=<seconds> has Bea watch the screen that long while both log every
// 2 s how it goes (Ana: captured, encoded, sent and why WebRTC holds it back; Bea: what
// arrives), then a line "measure sent: ..." and "measure received: ...".
//
// No microphone is opened; sounds play to the default output, which whatever runs the
// copies makes a silent one.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_webrtc/flutter_webrtc.dart' show StatsReport;
import 'package:fogata/main.dart' as app;
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:integration_test/integration_test.dart';

import 'keychain_dialog.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _dir = _env['FOGATA_SMOKE'] ?? Directory.systemTemp.path;
final _name = _role == 'a' ? 'Ana' : 'Bea';
final _otherName = _role == 'a' ? 'Bea' : 'Ana';

/// Whether this copy has a camera it may turn on: the made-up one on Linux, a real one
/// only where the run says so.
final _camera =
    _env['FOGATA_SMOKE_CAMERA'] == '1' ||
    (Platform.isLinux && _env['FOGATA_TEST_CAMERA'] == '1');

/// Whether the screen is picked in the system's own dialog (the desktop portal): asked of
/// the app at the start, as its share button asks.
late final bool _portal;

void _log(String line) => File('$_dir/$_role.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

/// Tells the other copy a step.
void _step(String name) {
  File('$_dir/$_role-$name').writeAsStringSync('done');
  _log('step: $name');
}

bool _other(String name) =>
    File('$_dir/${_role == 'a' ? 'b' : 'a'}-$name').existsSync();

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

/// Waits for the other copy's step.
Future<void> _after(WidgetTester tester, String step, {int seconds = 300}) =>
    _until(tester, '$_otherName: $step', () => _other(step), seconds: seconds);

Future<void> _tap(WidgetTester tester, Finder finder) async {
  await _until(tester, 'shown: $finder', () => finder.evaluate().isNotEmpty);
  await tester.ensureVisible(finder.first);
  await _wait(tester, const Duration(milliseconds: 200));
  await tester.tap(finder.first);
  await _wait(tester, const Duration(milliseconds: 300));
}

AppState _state(WidgetTester tester) =>
    AppScope.read(tester.element(find.byType(Shell)));

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the main things a person does', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      await _smoke(tester);
      File('$_dir/$_role-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      _log('failed: $e');
      File('$_dir/$_role-result.txt').writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

Future<void> _smoke(WidgetTester tester) async {
  _portal = await screenThroughPortal();
  // Whatever starts the copies may say which way the screen must go in its desktop
  // (FOGATA_SMOKE_SHARE: portal or list); the other way fails here, named.
  final expected = _env['FOGATA_SMOKE_SHARE'] ?? '';
  if (expected.isNotEmpty && expected != (_portal ? 'portal' : 'list')) {
    throw StateError(
      'the app would share the screen through '
      '${_portal ? 'the portal' : 'its own list'}, where this desktop needs '
      '${expected == 'portal' ? 'the portal' : 'its own list'}',
    );
  }
  _log(
    'start on ${Platform.operatingSystem}: '
    '${_portal ? 'the portal picks the screen' : 'the app lists the screens'}, '
    'camera ${_camera ? 'on' : 'none'}',
  );
  final state = await _profile(tester);
  final friend = await _friends(tester, state);
  await _direct(tester, state, friend);
  await _group(tester, state);
  await _room(tester, state);
  await _cameras(tester, state);
  await _screen(tester, state);
  await _file(tester, state, friend);
  await state.leaveRoom();
  await _until(tester, 'out of the room', () => state.roomState.room == null);
  _step('left');
  await _after(tester, 'left');
}

/// A new profile, from the welcome page.
Future<AppState> _profile(WidgetTester tester) async {
  await app.main();
  await _until(
    tester,
    'welcome',
    () => find.byType(TextField).evaluate().isNotEmpty,
  );
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      _name;
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  return _state(tester);
}

/// Ana gives Bea her friend link; Bea uses it and Ana accepts.
Future<UiPerson> _friends(WidgetTester tester, AppState state) async {
  if (_role == 'a') {
    final link = await state.makeInvitation(code: false);
    File('$_dir/link.txt').writeAsStringSync(link!.text);
    await _until(
      tester,
      'a friend request',
      () => state.contacts.requests.incoming.isNotEmpty,
      seconds: 300,
    );
    await state.answer(state.contacts.requests.incoming.first.person.id, true);
  } else {
    final file = File('$_dir/link.txt');
    await _until(
      tester,
      'the link',
      () => file.existsSync() && file.readAsStringSync().isNotEmpty,
      seconds: 300,
    );
    await state.useInvitation(file.readAsStringSync());
  }
  await _until(
    tester,
    'friends',
    () => state.contacts.friends.isNotEmpty,
    seconds: 300,
  );
  return state.contacts.friends.single.person;
}

bool _has(AppState state, String text) =>
    state.direct.page?.items.any((i) => i.text == text) ?? false;

/// Writes `text` in the chat shown and sends it, as a person would.
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

/// A message each way in the direct chat, opened from Friends.
Future<void> _direct(
  WidgetTester tester,
  AppState state,
  UiPerson friend,
) async {
  const hello = 'Hi, shall we try the call?';
  const reply = 'Yes, come into the group';
  state.go(Screen.friends);
  await _tap(tester, find.byKey(ValueKey('message:${friend.id}')));
  await _until(tester, 'the chat', () => state.direct.page != null);
  if (_role == 'a') {
    await _write(tester, hello);
    await _until(tester, 'the reply', () => _has(state, reply), seconds: 300);
  } else {
    await _until(tester, 'the message', () => _has(state, hello), seconds: 300);
    await _write(tester, reply);
    await _until(
      tester,
      'the reply delivered',
      () =>
          state.direct.page?.items.any((i) => i.text == reply && i.delivered) ??
          false,
    );
  }
  _step('chatted');
  await _after(tester, 'chatted');
}

/// Ana makes a group with Bea; Bea accepts the invitation.
Future<void> _group(WidgetTester tester, AppState state) async {
  if (_role == 'a') {
    await state.createGroup('Smoke', 3, [
      for (final friend in state.contacts.friends) friend.person.id,
    ]);
  } else {
    await _until(
      tester,
      'invited to the group',
      () => state.contacts.invitations.isNotEmpty,
      seconds: 300,
    );
    await state.answerInvitation(state.contacts.invitations.first.id, true);
  }
  await _until(
    tester,
    'in the group',
    () => state.contacts.groups.isNotEmpty,
    seconds: 300,
  );
}

/// Both enter the group's voice room, with the microphone off, and connect.
Future<void> _room(WidgetTester tester, AppState state) async {
  state.prefs.joinWithMic = false;
  await state.openRoom(state.contacts.groups.first.id);
  await _until(tester, 'in the call', () => state.callState.call != null);
  await _until(
    tester,
    'connected',
    () =>
        state.callState.call!.peers.isNotEmpty &&
        state.callState.call!.peers.values.every((p) => p.connected),
    seconds: 300,
  );
}

CallPeer _peer(AppState state) => state.callState.call!.peers.values.single;

/// Video frames decoded from the other copy so far.
Future<int> _frames(AppState state) async {
  var total = 0;
  for (final report
      in await _peer(state).connection?.getStats() ?? const <StatsReport>[]) {
    if (report.type == 'inbound-rtp' && report.values['kind'] == 'video') {
      total += (report.values['framesDecoded'] as num? ?? 0).toInt();
    }
  }
  return total;
}

/// The other copy's video keeps coming: frames decoded over a few seconds.
Future<void> _flowing(WidgetTester tester, AppState state, String what) async {
  final start = await _frames(state);
  await _wait(tester, const Duration(seconds: 4));
  final frames = await _frames(state) - start;
  _log('$what: $frames frames in 4 s');
  if (frames < 4) throw StateError('too few frames of the $what: $frames');
}

/// Each copy with a camera turns it on with the room's button; the other sees it.
Future<void> _cameras(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  if (_camera) {
    await _tap(tester, find.byTooltip('Turn the camera on'));
    await _until(tester, 'camera on', () => call.camera, seconds: 30);
  }
  _step(_camera ? 'camera' : 'no-camera');
  await _until(
    tester,
    '$_otherName said whether she has a camera',
    () => _other('camera') || _other('no-camera'),
    seconds: 300,
  );
  if (_other('camera')) {
    final peer = _peer(state);
    await _until(
      tester,
      "$_otherName's camera shown",
      () => peer.cam && peer.camera.videoWidth > 0,
      seconds: 60,
    );
    await _flowing(tester, state, 'camera');
  }
  _step('saw-camera');
  await _after(tester, 'saw-camera');
  if (_camera) {
    await _tap(tester, find.byTooltip('Turn the camera off'));
    await _until(tester, 'camera off', () => !call.camera, seconds: 30);
  }
  _step('camera-done');
  await _after(tester, 'camera-done');
}

/// Ana shares her screen through the room's button and Bea watches it.
Future<void> _screen(WidgetTester tester, AppState state) async {
  if (_role == 'a') {
    await _shareScreen(tester, state);
  } else {
    await _watchScreen(tester, state);
  }
}

/// With `FOGATA_SMOKE_MEASURE=<seconds>`, the shared screen is watched that long and both
/// copies log every 2 s how it goes: Ana what is captured, encoded and sent (and why WebRTC
/// holds it back), Bea what arrives; then a summary line each ("measure: ...").
final _measure = int.tryParse(_env['FOGATA_SMOKE_MEASURE'] ?? '') ?? 0;

/// The values of the first report of [type] that [match] accepts.
Map<dynamic, dynamic> _report(
  List<StatsReport> reports,
  String type,
  bool Function(Map<dynamic, dynamic>) match,
) {
  for (final r in reports) {
    if (r.type == type && match(r.values)) return r.values;
  }
  return const {};
}

double _num(Map<dynamic, dynamic> v, String key) =>
    (v[key] as num? ?? 0).toDouble();

/// Ana logs the screen she sends to Bea every 2 s while Bea measures it.
Future<void> _measureSent(WidgetTester tester, AppState state) async {
  final peer = _peer(state);
  Map<dynamic, dynamic>? before;
  var captured = 0.0, sent = 0.0, encodeMs = 0.0, samples = 0;
  final reasons = <String, int>{};
  while (!_other('saw-screen')) {
    await _wait(tester, const Duration(seconds: 2));
    final reports = await peer.connection!.getStats();
    final out = _report(
      reports,
      'outbound-rtp',
      (v) => '${v['mid']}' == '2' && v['kind'] == 'video',
    );
    final source = _report(
      reports,
      'media-source',
      (v) => v['kind'] == 'video',
    );
    if (out.isEmpty) continue;
    final reason = '${out['qualityLimitationReason']}';
    var line =
        'sent: ${_num(out, 'frameWidth').round()}x${_num(out, 'frameHeight').round()} '
        '${_num(out, 'framesPerSecond').toStringAsFixed(1)} fps out, '
        'captured ${_num(source, 'framesPerSecond').toStringAsFixed(1)} fps '
        '(${_num(source, 'width').round()}x${_num(source, 'height').round()}), '
        'limited by $reason, encoder ${out['encoderImplementation']}';
    if (before != null) {
      final frames = _num(out, 'framesEncoded') - _num(before, 'framesEncoded');
      final seconds =
          _num(out, 'totalEncodeTime') - _num(before, 'totalEncodeTime');
      final ms = frames > 0 ? 1000 * seconds / frames : 0.0;
      line +=
          ', ${frames.round()} frames encoded in 2 s at '
          '${ms.toStringAsFixed(1)} ms each, '
          '${(_num(out, 'framesSent') - _num(before, 'framesSent')).round()} sent';
      captured += _num(source, 'framesPerSecond');
      sent += _num(out, 'framesPerSecond');
      encodeMs += ms;
      samples++;
      reasons[reason] = (reasons[reason] ?? 0) + 1;
    }
    _log(line);
    before = out;
  }
  if (samples > 0) {
    final last = await peer.connection!.getStats();
    final out = _report(
      last,
      'outbound-rtp',
      (v) => '${v['mid']}' == '2' && v['kind'] == 'video',
    );
    _log(
      'measure sent: ${(captured / samples).toStringAsFixed(1)} fps captured, '
      '${(sent / samples).toStringAsFixed(1)} fps sent, '
      '${(encodeMs / samples).toStringAsFixed(1)} ms per frame encoded, '
      'limited by $reasons, durations ${out['qualityLimitationDurations']}, '
      'resolution changes ${out['qualityLimitationResolutionChanges']}',
    );
  }
}

/// Bea logs the screen she receives every 2 s for FOGATA_SMOKE_MEASURE seconds.
Future<void> _measureReceived(WidgetTester tester, AppState state) async {
  final peer = _peer(state);
  final end = DateTime.now().add(Duration(seconds: _measure));
  var fps = 0.0, samples = 0;
  var first = -1.0, decoded = 0.0;
  final started = DateTime.now();
  while (DateTime.now().isBefore(end)) {
    await _wait(tester, const Duration(seconds: 2));
    final inbound = _report(
      await peer.connection!.getStats(),
      'inbound-rtp',
      (v) => '${v['mid']}' == '2' && v['kind'] == 'video',
    );
    if (inbound.isEmpty) continue;
    decoded = _num(inbound, 'framesDecoded');
    if (first < 0) first = decoded;
    fps += _num(inbound, 'framesPerSecond');
    samples++;
    _log(
      'received: ${_num(inbound, 'frameWidth').round()}x'
      '${_num(inbound, 'frameHeight').round()} '
      '${_num(inbound, 'framesPerSecond').toStringAsFixed(1)} fps',
    );
  }
  final seconds = DateTime.now().difference(started).inMilliseconds / 1000;
  _log(
    'measure received: ${samples > 0 ? (fps / samples).toStringAsFixed(1) : '?'} fps '
    'on average, ${((decoded - first) / seconds).toStringAsFixed(1)} frames decoded '
    'a second over ${seconds.toStringAsFixed(0)} s',
  );
}

File get _pickerAnswer => File('${_env['SMOKE_PICKER']}/answer');

/// What the portal's picker did so far, a line each ("shown 1", "shared 1"...).
List<String> _dialogs() {
  final file = File('${_env['SMOKE_PICKER']}/dialogs');
  return file.existsSync() ? file.readAsLinesSync() : const [];
}

// Fresh finders each time: one kept across a dialog would still hold its closed elements.
Finder get _shareButton => find.byTooltip('Share the screen');
Finder get _appPicker => find.text('What do you want to share?');

/// Presses the share button twice in a row and cancels what opens; then presses it again
/// and shares the first screen. Fails if the app ends, if the second press opened a second
/// share, or if no screen goes out.
Future<void> _shareScreen(WidgetTester tester, AppState state) async {
  final call = state.callState.call!;
  // FOGATA_SMOKE_FPS asks the screen at that rate instead of the quality's own (30).
  final fps = int.tryParse(_env['FOGATA_SMOKE_FPS'] ?? '');
  if (fps != null) {
    await state.setScreenQuality(ScreenQuality(fps: fps));
    _log('screen asked at $fps fps');
  }
  if (_portal && _env['SMOKE_PICKER'] == null) {
    throw StateError(
      'under Wayland the test needs SMOKE_PICKER (the smoke session)',
    );
  }

  // How many screens and windows the app offers (it shows its list only for more than
  // one, and shares the only one at once): the system's list without Fogata's own
  // windows. Asked as the button asks, so a list that never answers fails here, named.
  var sources = 0;
  if (!_portal) {
    final asked = DateTime.now();
    final listed = await call.screens().timeout(
      const Duration(seconds: 30),
      onTimeout: () => throw StateError(
        'the system\'s list of screens and windows gave no answer in 30 s',
      ),
    );
    sources = listed.length;
    _log(
      'the system lists $sources sources in '
      '${DateTime.now().difference(asked).inMilliseconds} ms: '
      '${listed.map((s) => '${s.type.name}:${s.name}').join(' | ')}',
    );
    if (sources == 0) throw StateError('nothing to share');
  }

  // Twice in a row, then cancel.
  if (_portal) _pickerAnswer.writeAsStringSync('cancel');
  final before = _dialogs().length;
  await _until(
    tester,
    'the share button',
    () => _shareButton.evaluate().isNotEmpty,
  );
  // The list may already show when the second press comes: that press then lands beside
  // it and closes it, as a double click does.
  var seen = false;
  await tester.tap(_shareButton.first);
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 25));
    if (_appPicker.evaluate().isNotEmpty) seen = true;
  }
  if (_shareButton.evaluate().isNotEmpty) {
    await tester.tap(_shareButton.first, warnIfMissed: false);
  }
  _log('pressed share twice');
  if (_portal) {
    await _until(
      tester,
      'the portal dialog cancelled',
      () => _dialogs().skip(before).any((l) => l.startsWith('cancelled')),
      seconds: 60,
    );
    // A second dialog, if the second press opened one, is cancelled too.
    await _wait(tester, const Duration(seconds: 3));
    _log('portal: ${_dialogs().skip(before).join(', ')}');
  } else if (sources == 1) {
    // No list: the first press shares the only screen, the second may stop it again.
    await _wait(tester, const Duration(seconds: 5));
    _log(
      'one source, no list to cancel; sharing after two presses: ${call.sharing}',
    );
    if (call.sharing) {
      await _tap(tester, find.byTooltip('Stop sharing the screen'));
    }
  } else {
    final end = DateTime.now().add(const Duration(seconds: 60));
    var cancelled = 0;
    DateTime? quiet;
    while (DateTime.now().isBefore(end)) {
      await _wait(tester, const Duration(milliseconds: 300));
      if (_appPicker.evaluate().isNotEmpty) {
        seen = true;
        quiet = null;
        await tester.tap(find.text('Cancel').last);
        cancelled++;
        await _wait(tester, const Duration(milliseconds: 600));
        continue;
      }
      // Done once a list came and none has shown for two seconds.
      quiet ??= DateTime.now();
      if (seen && DateTime.now().difference(quiet).inSeconds >= 2) break;
    }
    _log(
      'app picker: ${seen ? 'shown' : 'never shown'}, cancelled $cancelled '
      '(the rest closed by the second press)',
    );
    if (!seen) {
      throw StateError(
        'the list of $sources screens and windows never showed after two '
        'presses; problem: ${call.problem}',
      );
    }
  }
  await _wait(tester, const Duration(seconds: 1));
  if (call.sharing) throw StateError('sharing after cancelling');

  // Once more, and share.
  if (_portal) _pickerAnswer.writeAsStringSync('share');
  final asked = _dialogs().length;
  await _tap(tester, _shareButton);
  if (!_portal) {
    await _wait(tester, const Duration(seconds: 1));
    final screen = find.textContaining('Screen: ');
    final end = DateTime.now().add(const Duration(seconds: 60));
    while (DateTime.now().isBefore(end) &&
        screen.evaluate().isEmpty &&
        !call.sharing) {
      await _wait(tester, const Duration(milliseconds: 300));
    }
    if (!call.sharing) {
      _log('picking the first screen in the list');
      await _tap(tester, screen);
    }
  }
  await _until(tester, 'sharing the screen', () => call.sharing, seconds: 90);
  if (_portal) {
    // The picker notes its answer once the dialog has gone, a moment after the screen came.
    await _until(
      tester,
      'shared through the portal dialog',
      () => _dialogs().skip(asked).any((l) => l.startsWith('shared')),
      seconds: 10,
    );
    _log('portal: ${_dialogs().skip(asked).join(', ')}');
  }
  _step('sharing');
  if (_measure > 0) {
    await _measureSent(tester, state);
  } else {
    await _after(tester, 'saw-screen');
  }
  await _tap(tester, find.byTooltip('Stop sharing the screen'));
  await _until(tester, 'stopped sharing', () => !call.sharing, seconds: 30);
  _step('stopped');
  await _after(tester, 'screen-gone');
}

/// Bea watches Ana's screen from its card and sees it come.
Future<void> _watchScreen(WidgetTester tester, AppState state) async {
  await _after(tester, 'sharing', seconds: 600);
  final peer = _peer(state);
  await _until(
    tester,
    "$_otherName's screen offered",
    () => peer.sharing && find.text('Watch').evaluate().isNotEmpty,
    seconds: 60,
  );
  await _tap(tester, find.text('Watch'));
  await _until(
    tester,
    "$_otherName's screen shown",
    () => peer.screen.videoWidth > 0,
    seconds: 60,
  );
  _log('screen ${peer.screen.videoWidth}x${peer.screen.videoHeight}');
  await _flowing(tester, state, 'screen');
  if (_measure > 0) await _measureReceived(tester, state);
  _step('saw-screen');
  await _after(tester, 'stopped');
  await _until(
    tester,
    "$_otherName's screen gone",
    () => !peer.sharing,
    seconds: 30,
  );
  _step('screen-gone');
}

/// Ana sends Bea a file in their direct chat; Bea downloads it.
Future<void> _file(WidgetTester tester, AppState state, UiPerson friend) async {
  const words = 'The budget';
  final group = state.roomState.roomGroup;
  state.go(Screen.friends);
  await _tap(tester, find.byKey(ValueKey('message:${friend.id}')));
  await _until(tester, 'the chat', () => state.direct.page != null);
  if (_role == 'a') {
    final pdf = '$_dir/budget.pdf';
    File(pdf).writeAsBytesSync(List.filled(300000, 37));
    await state.direct.addOutgoing([pdf]);
    await _wait(tester, const Duration(milliseconds: 300));
    await _write(tester, words);
    await _until(
      tester,
      'the file delivered',
      () =>
          state.direct.page?.items.any((i) => i.text == words && i.delivered) ??
          false,
      seconds: 180,
    );
    await _after(tester, 'got-file', seconds: 300);
  } else {
    UiDirectItem? sent() {
      for (final i in state.direct.page?.items ?? const <UiDirectItem>[]) {
        if (i.text == words && i.files.isNotEmpty) return i;
      }
      return null;
    }

    await _until(
      tester,
      'the file offered',
      () => sent() != null,
      seconds: 300,
    );
    final pdf = sent()!.files.single;
    if (pdf.state != UiFileState.here) {
      await _tap(tester, find.byKey(ValueKey('direct-get:${pdf.id}')));
    }
    await _until(
      tester,
      'the file here',
      () => sent()?.files.single.state == UiFileState.here,
      seconds: 180,
    );
    _step('got-file');
  }
  // Back to the room, to leave it from there.
  if (group != null) state.go(Screen.room);
  await _wait(tester, const Duration(seconds: 1));
}
