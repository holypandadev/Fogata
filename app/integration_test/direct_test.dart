// Direct messages between two copies of the app on one computer. Build once with
//
//   flutter build linux --debug -t integration_test/direct_test.dart
//
// and start copy a (Ana) and copy b (Bea) with FOGATA_ROLE=a or b, a FOGATA_DATA_DIR each,
// and the same FOGATA_DIRECT folder, where they leave each other the friend link and
// their steps. Copy b runs twice: FOGATA_PHASE=1, then, once it has quit, FOGATA_PHASE=2 on
// the same data folder; meanwhile Ana cannot write, and writes once Bea is back.
// FOGATA_CLIP is a short video to send; FOGATA_SHOTS, a folder for pictures of the window.
// Each run writes <role><phase>.log and <role><phase>-result.txt ("passed" or why it
// failed) to FOGATA_DIRECT. The sounds go to the default output: run the copies with
// PULSE_SINK set to a null sink.

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
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart' show bannerColors;
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _phase = _env['FOGATA_PHASE'] ?? '1';
final _dir = _env['FOGATA_DIRECT'] ?? Directory.systemTemp.path;

void _log(String line) => File('$_dir/$_role$_phase.log').writeAsStringSync(
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
    await _wait(tester, const Duration(milliseconds: 200));
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

/// What the line under the chat says about who is typing; empty while nobody is.
String _typingLine(WidgetTester tester) {
  final found = find.byKey(const ValueKey('typing-line'));
  if (found.evaluate().isEmpty) return '';
  return tester.widget<Text>(found.first).textSpan!.toPlainText();
}

/// The chat shown holds a message with `text`.
bool _has(AppState state, String text) =>
    state.direct.page?.items.any((i) => i.text == text) ?? false;

/// A picture to send: a dusk-coloured gradient, as a PNG.
Future<String> _picture() async {
  final recorder = ui.PictureRecorder();
  const size = Size(900, 600);
  ui.Canvas(recorder).drawRect(
    Offset.zero & size,
    Paint()
      ..shader = ui.Gradient.linear(Offset.zero, Offset(0, size.height), [
        const Color(0xFF2B3A67),
        const Color(0xFFB4532A),
      ]),
  );
  final image = await recorder.endRecording().toImage(900, 600);
  final png = await image.toByteData(format: ui.ImageByteFormat.png);
  final path = '$_dir/lake.png';
  File(path).writeAsBytesSync(png!.buffer.asUint8List());
  return path;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('direct messages between two copies', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      if (_role == 'a') {
        await _ana(tester);
      } else if (_phase == '1') {
        await _bea(tester);
      } else {
        await _beaAgain(tester);
      }
      File('$_dir/$_role$_phase-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$_role$_phase-result.txt')
          .writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

/// A new profile, from the welcome page.
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
  return _state(tester);
}

Future<void> _ana(WidgetTester tester) async {
  final state = await _welcome(tester, 'Ana');
  final link = await state.makeInvitation(code: false);
  File('$_dir/link.txt').writeAsStringSync(link!.text);
  await _until(
    tester,
    'a friend request',
    () => state.contacts.requests.incoming.isNotEmpty,
    seconds: 300,
  );
  await state.answer(state.contacts.requests.incoming.first.person.id, true);
  await _until(
    tester,
    'friends with Bea',
    () => state.contacts.friends.isNotEmpty,
  );
  final bea = state.contacts.friends.single.person;

  // From Friends: Message opens the chat, empty, and Ana writes first.
  state.go(Screen.friends);
  await _tap(tester, find.byKey(ValueKey('message:${bea.id}')));
  await _until(tester, 'the chat', () => state.direct.page != null);
  await _shot(tester, 'a1-new-chat');
  await _write(tester, 'Hi Bea, how was the weekend?');
  // Bea writes her answer: it shows under the chat until it comes.
  await _until(
    tester,
    'Bea typing',
    () => _typingLine(tester) == 'Bea is typing…',
    seconds: 300,
  );
  await _shot(tester, 'a1b-typing');
  _step('saw-typing');
  await _until(
    tester,
    'the reply',
    () => _has(state, 'Great, can you send me the photos?'),
  );
  final replied = DateTime.now();
  await _until(
    tester,
    'Bea no longer typing',
    () => _typingLine(tester).isEmpty,
    seconds: 3,
  );
  _log(
    'typing gone ${DateTime.now().difference(replied).inMilliseconds} ms after the reply',
  );
  await _shot(tester, 'a2-reply');

  // Ana pins Bea's reply with the button over it; it shows pinned on both.
  final reply = state.direct.page!.items.firstWhere(
    (i) => i.text == 'Great, can you send me the photos?',
  );
  final hover = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await hover.addPointer(location: Offset.zero);
  // Over the message in the chat (the list beside shows its words too).
  await hover.moveTo(
    tester.getCenter(
      find.descendant(
        of: find.byKey(ValueKey('direct-item:${reply.id}')),
        matching: find.text(reply.text),
      ),
    ),
  );
  await _wait(tester, const Duration(milliseconds: 400));
  await _shot(tester, 'a2-pin-button');
  await tester.tap(find.byKey(ValueKey('direct-pin:${reply.id}')));
  await hover.moveTo(Offset.zero);
  await hover.removePointer();
  await _until(
    tester,
    'pinned here',
    () => state.direct.page?.pinned.any((i) => i.id == reply.id) ?? false,
  );
  await _until(
    tester,
    'the pin delivered',
    () =>
        state.direct.page?.items.any(
          (i) => i.pin?.target == reply.id && i.delivered,
        ) ??
        false,
  );
  await _shot(tester, 'a2-pinned');
  await _tap(tester, find.byKey(const ValueKey('direct-pins')));
  await _until(
    tester,
    'the pinned list',
    () => find.byKey(ValueKey('pinned:${reply.id}')).evaluate().isNotEmpty,
  );
  await _shot(tester, 'a2-pins-list');
  await tester.sendKeyEvent(LogicalKeyboardKey.escape);
  await _wait(tester, const Duration(milliseconds: 600));

  // Bea's name in the header opens her card, as in Friends.
  await _tap(tester, find.byKey(const ValueKey('direct-friend')));
  await _until(
    tester,
    'Bea\'s card',
    () =>
        find.byKey(const ValueKey('friend-card-message')).evaluate().isNotEmpty,
  );
  await _shot(tester, 'a2-card');
  state.closeFriendCard();
  await _wait(tester, const Duration(milliseconds: 400));

  // Bea changes her banner: the chat's side panel follows.
  await _until(tester, 'Bea\'s new banner', () => _other('banner'));
  await _until(
    tester,
    'the new banner beside the chat',
    () => state.direct.banners[bea.id]?.color == bannerColors[2].$1.toARGB32(),
  );
  await _shot(tester, 'a2-banner');

  // Pictures, a video and a file, with a few words.
  final files = [await _picture()];
  final clip = _env['FOGATA_CLIP'];
  if (clip != null) files.add(clip);
  final pdf = '$_dir/budget.pdf';
  File(pdf).writeAsBytesSync(List.filled(220000, 37));
  files.add(pdf);
  await state.direct.addOutgoing(files);
  await _wait(tester, const Duration(milliseconds: 300));
  await _shot(tester, 'a3-files-waiting');
  await _write(tester, 'Here they are');
  await _until(
    tester,
    'the files delivered',
    () =>
        state.direct.page?.items.any(
          (i) => i.text == 'Here they are' && i.delivered,
        ) ??
        false,
  );
  await _shot(tester, 'a4-files-sent');

  // The disappearing timer, for both.
  await _tap(tester, find.byKey(const ValueKey('direct-timer')));
  await _shot(tester, 'a5-timer-menu');
  await _tap(tester, find.byKey(ValueKey('timer:${7 * 24 * 3600 * 1000}')));
  await _until(
    tester,
    'the timer on',
    () => state.direct.page?.timerMs == 7 * 24 * 3600 * 1000,
  );
  await _shot(tester, 'a6-timer-on');
  _step('timer');

  // Appearing offline, Ana cannot write: the box says why.
  await state.setAppearOffline(true);
  await _until(
    tester,
    'the box closed while appearing offline',
    () => find
        .text('You appear offline. Go online to write.')
        .evaluate()
        .isNotEmpty,
  );
  await _shot(tester, 'a6b-appear-offline');
  await state.setAppearOffline(false);
  await _until(
    tester,
    'the box open again',
    () => find.byKey(const ValueKey('direct-message')).evaluate().isNotEmpty,
  );

  // Bea goes away: Ana can read the chat but not write in it.
  await _until(tester, 'Bea gone', () => _other('quit'), seconds: 300);
  await _until(
    tester,
    'Bea offline',
    () => !(state.direct.page?.online ?? true),
    seconds: 120,
  );
  await _until(
    tester,
    'the box closed while Bea is offline',
    () =>
        find.byKey(const ValueKey('direct-locked')).evaluate().isNotEmpty &&
        find.byKey(const ValueKey('direct-message')).evaluate().isEmpty,
  );
  if (find
      .text('Bea is offline. You can write when you’re both online.')
      .evaluate()
      .isEmpty) {
    throw StateError('the closed box does not say why');
  }
  await _shot(tester, 'a7-offline');
  _step('wrote');

  // Bea is back: Ana writes again, and it goes at once.
  await _until(
    tester,
    'Bea back',
    () => find.byKey(const ValueKey('direct-message')).evaluate().isNotEmpty,
    seconds: 600,
  );
  await _write(tester, 'Are you still there?');
  await _until(
    tester,
    'delivered once Bea is back',
    () =>
        state.direct.page?.items.any(
          (i) => i.text == 'Are you still there?' && i.delivered,
        ) ??
        false,
    seconds: 600,
  );
  await _shot(tester, 'a8-delivered');
  // With the timer on, hovering the message tells when it goes from this device.
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: Offset.zero);
  await mouse.moveTo(tester.getCenter(find.text('Are you still there?').first));
  await _until(
    tester,
    'when it disappears',
    () => find.text('Disappears in 7 days').evaluate().isNotEmpty,
    seconds: 10,
  );
  await _shot(tester, 'a8-disappears');
  await mouse.moveTo(Offset.zero);
  await mouse.removePointer();
  await _until(tester, 'Bea read it', () => _other('read'), seconds: 300);
  // Bea unpinned the reply after her restart: unpinned here too.
  await _until(
    tester,
    'unpinned by Bea',
    () => state.direct.page?.pinned.isEmpty ?? false,
  );
  await _shot(tester, 'a8-unpinned');

  // No longer friends: the chat stays, read-only.
  await state.removeFriend(bea.id);
  await _until(
    tester,
    'the chat read-only',
    () => state.direct.page?.endedMs != null,
  );
  await _shot(tester, 'a9-ended');
  _step('removed');
  await _until(tester, 'Bea done', () => _other('done'), seconds: 300);
}

Future<void> _bea(WidgetTester tester) async {
  final state = await _welcome(tester, 'Bea');
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
    'friends with Ana',
    () => state.contacts.friends.isNotEmpty,
    seconds: 300,
  );
  final ana = state.contacts.friends.single.person;

  // Ana's message comes while Bea is on Home: the list shows it unread, with a sound.
  await _until(
    tester,
    'Ana\'s message unread',
    () => state.direct.chat(ana.id)?.unread == 1,
    seconds: 300,
  );
  await _shot(tester, 'b1-unread');
  if (!state.played.contains(UiSound.message)) {
    throw StateError('no sound for the message');
  }
  await _tap(tester, find.byKey(ValueKey('direct-row:${ana.id}')));
  await _until(
    tester,
    'the chat',
    () => _has(state, 'Hi Bea, how was the weekend?'),
  );
  await _until(tester, 'read', () => state.direct.chat(ana.id)?.unread == 0);
  // Writing for a while before sending: Ana sees it.
  final box = find.descendant(
    of: find.byKey(const ValueKey('direct-message')),
    matching: find.byType(TextField),
  );
  final words = 'Great, can you send'.split(' ');
  var n = 0;
  while (!_other('saw-typing')) {
    tester.widget<TextField>(box).controller!.text = words
        .take(n++ % words.length + 1)
        .join(' ');
    await _wait(tester, const Duration(seconds: 1));
    if (n > 300) throw StateError('Ana never saw Bea typing');
  }
  await _shot(tester, 'b1b-typing');
  await _write(tester, 'Great, can you send me the photos?');

  // Ana pins the reply: it shows pinned here, with a line that brings it into view.
  await _until(
    tester,
    'Ana pinned the reply',
    () =>
        state.direct.page?.pinned.any(
          (i) => i.text == 'Great, can you send me the photos?',
        ) ??
        false,
  );
  await _shot(tester, 'b1-pinned');
  await _tap(tester, find.text('Ana pinned a message'));
  await _shot(tester, 'b1-pin-line');
  // A new banner colour, for Ana's page of this chat to follow.
  await state.setBannerColor(bannerColors[2].$1);
  _step('banner');

  // The picture comes by itself; the video and the file wait to be opened.
  UiDirectItem? sent() {
    for (final i in state.direct.page?.items ?? const <UiDirectItem>[]) {
      if (i.text == 'Here they are') return i;
    }
    return null;
  }

  await _until(
    tester,
    'the picture here',
    () =>
        sent()?.files.any(
          (f) => f.kind == UiFileKind.picture && f.state == UiFileState.here,
        ) ??
        false,
    seconds: 180,
  );
  await _shot(tester, 'b2-picture');
  final pdf = sent()!.files.firstWhere((f) => f.name == 'budget.pdf');
  if (pdf.state != UiFileState.missing) {
    throw StateError('the file came without being opened: ${pdf.state}');
  }
  await _tap(tester, find.byKey(ValueKey('direct-get:${pdf.id}')));
  for (final video in sent()!.files.where((f) => f.kind == UiFileKind.video)) {
    await _tap(tester, find.byKey(ValueKey('direct-get:${video.id}')));
  }
  await _until(
    tester,
    'everything here',
    () => sent()!.files.every((f) => f.state == UiFileState.here),
  );
  await _shot(tester, 'b3-all-here');
  // The video plays over the window with this computer's own player.
  for (final video in sent()!.files.where((f) => f.kind == UiFileKind.video)) {
    await _tap(tester, find.byKey(ValueKey('direct-video:${video.id}')));
    await _wait(tester, const Duration(seconds: 2));
    await _shot(tester, 'b3-video');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _wait(tester, const Duration(milliseconds: 600));
  }

  // The timer Ana set shows here too.
  await _until(tester, 'Ana\'s timer', () => _other('timer'), seconds: 300);
  await _until(
    tester,
    'the timer here',
    () => state.direct.page?.timerMs == 7 * 24 * 3600 * 1000,
  );
  await _shot(tester, 'b4-timer');

  // "Free up space…" opens the settings at Storage: clear the chat's files; the messages
  // stay and say so.
  await _tap(tester, find.byKey(const ValueKey('direct-free')));
  await _until(
    tester,
    'storage',
    () => find.byKey(ValueKey('clear-files:${ana.id}')).evaluate().isNotEmpty,
  );
  await _shot(tester, 'b5-storage');
  await _tap(tester, find.byKey(ValueKey('clear-files:${ana.id}')));
  await _until(
    tester,
    'cleared',
    () => find.text('Cleared').evaluate().isNotEmpty,
  );
  await _shot(tester, 'b6-cleared');
  state.closeSettings();
  await _until(
    tester,
    'removed here',
    () => sent()!.files.every((f) => f.state == UiFileState.removed),
  );
  await _shot(tester, 'b7-removed');
  await _tap(tester, find.byKey(ValueKey('direct-again:${pdf.id}')));
  await _until(
    tester,
    'asked again',
    () => sent()!.files.any(
      (f) => f.name == pdf.name && f.state == UiFileState.here,
    ),
  );
  await _shot(tester, 'b8-asked-again');
  _step('quit');
  await state.leaveBeforeQuit();
  await _wait(tester, const Duration(seconds: 1));
}

/// Bea starts again, later: Ana, who could not write while she was away, writes once she
/// is back.
Future<void> _beaAgain(WidgetTester tester) async {
  await app.main();
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  await _until(tester, 'Ana wrote', () => _other('wrote'), seconds: 600);
  await _until(
    tester,
    'Ana\'s message once back',
    () => state.direct.chats.any((c) => c.last == 'Are you still there?'),
    seconds: 600,
  );
  final ana = state.direct.chats.first.friend;
  await _tap(tester, find.byKey(ValueKey('direct-row:${ana.id}')));
  await _until(tester, 'the chat', () => _has(state, 'Are you still there?'));
  // Kept from before the restart too, the pin with it.
  if (!_has(state, 'Hi Bea, how was the weekend?')) {
    throw StateError('the earlier messages are gone');
  }
  await _shot(tester, 'b9-back');
  final pinned = state.direct.page!.pinned;
  if (pinned.length != 1 ||
      pinned.single.text != 'Great, can you send me the photos?') {
    throw StateError('the pin did not last: ${pinned.map((i) => i.text)}');
  }
  await _tap(tester, find.byKey(const ValueKey('direct-pins')));
  await _shot(tester, 'b9-pins-list');
  // Bea unpins it from the list.
  await _tap(tester, find.byKey(ValueKey('unpin:${pinned.single.id}')));
  await _until(
    tester,
    'unpinned',
    () => state.direct.page?.pinned.isEmpty ?? false,
  );
  await _shot(tester, 'b9-unpinned');
  await tester.sendKeyEvent(LogicalKeyboardKey.escape);
  await _wait(tester, const Duration(milliseconds: 600));
  // Ana's card from her name, in this theme too.
  await _tap(tester, find.byKey(const ValueKey('direct-friend')));
  await _until(
    tester,
    'Ana\'s card',
    () =>
        find.byKey(const ValueKey('friend-card-message')).evaluate().isNotEmpty,
  );
  await _shot(tester, 'b9-card');
  state.closeFriendCard();
  await _wait(tester, const Duration(milliseconds: 400));
  _step('read');

  await _until(
    tester,
    'Ana removed Bea',
    () => _other('removed'),
    seconds: 300,
  );
  await _until(
    tester,
    'the chat read-only',
    () => state.direct.page?.endedMs != null,
    seconds: 120,
  );
  await _shot(tester, 'b10-ended');
  await _tap(tester, find.byKey(const ValueKey('direct-delete')));
  await _shot(tester, 'b11-delete-asked');
  await _tap(tester, find.text('Delete'));
  await _until(tester, 'deleted', () => state.direct.chats.isEmpty);
  await _shot(tester, 'b12-deleted');
  _step('done');
}
