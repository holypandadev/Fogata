// Away by itself, between copies of the app on one computer. Build once with
//
//   flutter build linux --debug -t integration_test/away_test.dart
//
// and start copy a (Ana) and copy b (Bea) with FOGATA_ROLE=a or b, a FOGATA_DATA_DIR each,
// and the same FOGATA_AWAY folder, where they leave each other the friend link and their
// steps. They become friends and share a group; then nobody touches Bea's window, and with
// FOGATA_TEST_AWAY_AFTER_MS (debug builds only) she is away after that long. Ana sees her
// away in Friends, the direct chats, the chat's header and the group's members; Bea moves
// the mouse, and Ana sees her online again.
//
// FOGATA_ROLE=c (Leo) meets an older command-line copy instead: tool/peer.sh-like Marta
// whose output folder is FOGATA_PEER. Leo goes away and comes back while Marta stays
// connected.
//
// FOGATA_SHOTS is a folder for pictures of the window. Each run writes <role>.log and
// <role>-result.txt ("passed" or why it failed) to FOGATA_AWAY.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _dir = _env['FOGATA_AWAY'] ?? Directory.systemTemp.path;

void _log(String line) => File('$_dir/$_role.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

/// Tells the other copy a step, with when it happened.
void _step(String name) {
  File('$_dir/$_role-$name')
      .writeAsStringSync('${DateTime.now().millisecondsSinceEpoch}');
  _log('step: $name');
}

File _otherStep(String name) => File('$_dir/${_role == 'a' ? 'b' : 'a'}-$name');

bool _other(String name) => _otherStep(name).existsSync();

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
      await _shot(tester, '$_role-failed');
      throw StateError('not in time: $what');
    }
    await _wait(tester, const Duration(milliseconds: 100));
  }
  _log(what);
}

AppState _state(WidgetTester tester) =>
    AppScope.read(tester.element(find.byType(Shell)));

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('away by itself', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    try {
      switch (_role) {
        case 'a':
          await _ana(tester);
        case 'b':
          await _bea(tester);
        default:
          await _leo(tester);
      }
      File('$_dir/$_role-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$_role-result.txt').writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

/// A new profile, from the welcome page: the only time the window is touched.
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
  await tester.tap(find.text('Create'));
  await _until(tester, 'home', () => find.byType(Shell).evaluate().isNotEmpty);
  final state = _state(tester);
  state.prefs.sounds = false;
  state.prefs.joinWithMic = false;
  return state;
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
  final group = await state.createGroup('The usual crew', 3, [bea.id]);
  await _until(
    tester,
    'Bea in the group',
    () =>
        state.contacts.members[group]?.any(
          (m) => m.person.id == bea.id && m.path != null,
        ) ??
        false,
    seconds: 300,
  );
  // A line in the chat, so that it is listed beside.
  await state.direct.show(bea.id);
  await _until(tester, 'the chat', () => state.direct.page != null);
  await state.direct.send('you there?');
  await _until(
    tester,
    'the line delivered',
    () => state.direct.page?.items.any((i) => i.delivered) ?? false,
  );

  bool beaIs(UiPresence p) => state.contacts.friends.single.presence == p;
  bool memberAway() =>
      state.contacts.members[group]?.any(
        (m) => m.person.id == bea.id && m.away,
      ) ??
      false;
  state.go(Screen.friends);
  await _until(tester, 'Bea ready', () => _other('ready'), seconds: 300);
  await _until(tester, 'Bea away', () => beaIs(UiPresence.away));
  final ready = int.parse(_otherStep('ready').readAsStringSync());
  _log(
    'Bea away ${DateTime.now().millisecondsSinceEpoch - ready} ms after her last input',
  );
  await _until(tester, 'Bea away among the members', memberAway);
  await _until(
    tester,
    'Bea away in the chats',
    () => state.direct.chats.any((c) => c.friend.id == bea.id && c.away),
  );
  // She still counts as online.
  expect(state.contacts.group(group)?.online, 1);
  await _shot(tester, 'a2-friends-away');
  await state.direct.show(bea.id);
  await _until(tester, 'the chat says away', () => state.direct.page!.away);
  await _shot(tester, 'a3-direct-away');
  state.showMembers(group!);
  await _shot(tester, 'a3b-members-away');
  await state.setTheme(ThemeMode.light);
  state.go(Screen.friends);
  await _shot(tester, 'a2b-friends-away-light');
  await state.setTheme(ThemeMode.dark);
  _step('seen-away');

  await _until(tester, 'Bea moved', () => _other('moved'), seconds: 120);
  await _until(tester, 'Bea online again', () => beaIs(UiPresence.online));
  final moved = int.parse(_otherStep('moved').readAsStringSync());
  _log(
    'Bea online ${DateTime.now().millisecondsSinceEpoch - moved} ms after she moved',
  );
  await _until(
    tester,
    'Bea no longer away among the members',
    () => !memberAway(),
  );
  await _shot(tester, 'a4-friends-back');
  _step('done');
  await _until(tester, 'Bea done', () => _other('done'), seconds: 120);
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
  await _until(
    tester,
    'invited to the group',
    () => state.contacts.invitations.isNotEmpty,
    seconds: 300,
  );
  await state.answerInvitation(state.contacts.invitations.first.id, true);
  await _until(tester, 'in the group', () => state.contacts.groups.isNotEmpty);
  // The last input was the tap on Create: nothing touches the window from here on.
  _step('ready');
  await _until(tester, 'away', () => state.contacts.away);
  expect(find.text('Away'), findsWidgets);
  // The panel, opened without a click (a click would bring her back).
  state.openUserPanel(tester.getRect(find.byKey(const ValueKey('own-pill'))));
  await _shot(tester, 'a1-own-away');
  state.closeUserPanel();
  await _until(tester, 'Ana saw it', () => _other('seen-away'), seconds: 300);
  expect(state.contacts.away, isTrue, reason: 'still away, untouched');

  // Bea moves the mouse over the window.
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: const Offset(700, 450));
  await mouse.moveTo(const Offset(720, 460));
  _step('moved');
  await _until(tester, 'back', () => !state.contacts.away, seconds: 10);
  await _shot(tester, 'b-back');
  await mouse.removePointer();
  await _until(tester, 'Ana done', () => _other('done'), seconds: 120);
  _step('done');
}

/// Marta's friend link, from the older copy's output.
Future<String> _martasLink() async {
  final file = File('${_env['FOGATA_PEER']}/invite.out');
  final end = DateTime.now().add(const Duration(minutes: 5));
  while (DateTime.now().isBefore(end)) {
    if (file.existsSync()) {
      final match = RegExp(r'fogata:friend:\S+')
          .firstMatch(file.readAsStringSync());
      if (match != null) return match.group(0)!;
    }
    await Future<void>.delayed(const Duration(milliseconds: 300));
  }
  throw StateError('the older copy showed no link');
}

Future<void> _leo(WidgetTester tester) async {
  final state = await _welcome(tester, 'Leo');
  await state.useInvitation(await _martasLink());
  await _until(
    tester,
    'friends with Marta',
    () => state.contacts.friends.isNotEmpty,
    seconds: 300,
  );
  final marta = state.contacts.friends.single.person;
  final group = await state.createGroup('The usual crew', 3, [marta.id]);
  bool connected() =>
      state.contacts.friends.single.presence != UiPresence.offline &&
      (state.contacts.members[group]?.any(
            (m) => m.person.id == marta.id && m.path != null,
          ) ??
          false);
  await _until(tester, 'Marta in the group', connected, seconds: 300);
  _step('ready');
  await _until(tester, 'away', () => state.contacts.away);
  // Marta's older copy gets the away status and presence it cannot read: she stays.
  for (var i = 0; i < 30; i++) {
    await _wait(tester, const Duration(seconds: 1));
    if (!connected()) throw StateError('Marta went while Leo was away');
  }
  await _shot(tester, 'c-away-with-an-older-copy');
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: const Offset(700, 450));
  await mouse.moveTo(const Offset(720, 460));
  await _until(tester, 'back', () => !state.contacts.away, seconds: 10);
  await mouse.removePointer();
  for (var i = 0; i < 15; i++) {
    await _wait(tester, const Duration(seconds: 1));
    if (!connected()) throw StateError('Marta went after Leo came back');
  }
  _step('done');
}
