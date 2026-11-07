// Moving an account between two copies of the app on one computer. Build once with
//
//   flutter build linux --debug -t integration_test/move_test.dart
//
// and start copy a (Ana's computer, with her account) and copy b (a new computer) with
// FOGATA_ROLE=a or b, a FOGATA_DATA_DIR each and the same FOGATA_MOVE folder, where b
// leaves the code it shows. Copy a makes Ana and a group, then moves her account from
// Settings → Move account; copy b brings it from the welcome page and opens the app as
// Ana. FOGATA_SHOTS, a folder for pictures of the window. Each copy writes <role>.log and
// <role>-result.txt ("passed" or why it failed) to FOGATA_MOVE.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/main.dart' as app;

import 'keychain_dialog.dart';

import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/move_account.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:integration_test/integration_test.dart';

final _env = Platform.environment;
final _role = _env['FOGATA_ROLE'] ?? 'a';
final _dir = _env['FOGATA_MOVE'] ?? Directory.systemTemp.path;

/// `move` (the whole move from the welcome page), `replace` (the new computer has a
/// profile of its own, replaced from Settings), `lost` (the old computer loses the last
/// message on purpose, FOGATA_TEST_LOSE_COMMIT=1, and the move is finished with a new
/// code) or `keep` (the same, then the new computer throws it away and the old one keeps
/// using the account).
final _scenario = _env['FOGATA_SCENARIO'] ?? 'move';

void _log(String line) => File('$_dir/$_role.log').writeAsStringSync(
  '${DateTime.now().toIso8601String()} $line\n',
  mode: FileMode.append,
);

Future<void> _shot(WidgetTester tester, String name) async {
  await _wait(tester, const Duration(milliseconds: 700));
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

bool _shown(Finder finder) => finder.evaluate().isNotEmpty;

const _crockford = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/// The check character of nine code characters, as the core works it out.
String _check(String nine) {
  var sum = 0;
  for (var i = 0; i < 9; i++) {
    sum += (2 * i + 1) * _crockford.indexOf(nine[i]);
  }
  return _crockford[sum % 32];
}

bool _wellFormed(String code) {
  final chars = code.replaceAll('-', '');
  return chars.length == 10 && _check(chars.substring(0, 9)) == chars[9];
}

/// Another well-formed code with the same first characters (where the two meet).
String _otherCode(String code) {
  final chars = code.replaceAll('-', '').split('');
  chars[6] = chars[6] == '0' ? '1' : '0';
  chars[9] = _check(chars.sublist(0, 9).join());
  final text = chars.join();
  return '${text.substring(0, 5)}-${text.substring(5)}';
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('moving an account between two copies', (tester) async {
    if (_env['FOGATA_DATA_DIR'] == null) {
      fail('FOGATA_DATA_DIR is not set; the test needs a folder of its own');
    }
    // A copy started again by the one it replaced (see _replacing) reports apart.
    final restarted =
        _role == 'b' &&
        File('${_env['FOGATA_DATA_DIR']}/moved-in.json').existsSync();
    final name = restarted ? 'b-restarted' : _role;
    try {
      switch ((_scenario, _role)) {
        case ('move', 'a'):
          await _old(tester);
        case ('move', _):
          await _new(tester);
        case ('replace', 'a') || ('lost', 'a') || ('keep', 'a'):
          await _oldThen(tester);
        case ('replace', _):
          await (restarted ? _replaced(tester) : _replacing(tester));
        case ('lost', _) || ('keep', _):
          await _newLosing(tester);
      }
      File('$_dir/$name-result.txt').writeAsStringSync('passed\n');
    } catch (e, trace) {
      File('$_dir/$name-result.txt').writeAsStringSync('failed: $e\n$trace\n');
      rethrow;
    }
  });
}

/// Ana's computer: her profile and a group, then Settings → Move account.
Future<void> _old(WidgetTester tester) async {
  await app.main();
  await _until(tester, 'welcome', () => _shown(find.byType(TextField)));
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      'Ana';
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => _shown(find.byType(Shell)));
  final state = AppScope.read(tester.element(find.byType(Shell)));
  await state.createGroup('The usual crew', 2, const []);
  await _until(tester, 'the group', () => state.contacts.groups.isNotEmpty);

  state.openSettings(SettingsSection.move);
  await _until(
    tester,
    'the move page',
    () => _shown(find.byKey(const ValueKey('move-code-field'))),
  );
  await _shot(tester, 'a1-settings');
  final file = File('$_dir/code.txt');
  await _until(tester, 'the code from b', file.existsSync, seconds: 300);
  final code = file.readAsStringSync().trim();
  final field = find.descendant(
    of: find.byKey(const ValueKey('move-code-field')),
    matching: find.byType(TextField),
  );
  Future<void> type(String text) async {
    tester.widget<TextField>(field).controller!.text = text;
    await _wait(tester, const Duration(milliseconds: 300));
    await _tap(tester, find.byKey(const ValueKey('move-continue')));
  }

  // A typo is told at once, with nothing looked for.
  await type('${code.substring(0, 10)}${code[10] == 'A' ? 'B' : 'A'}');
  if (_wellFormed('${code.substring(0, 10)}${code[10] == 'A' ? 'B' : 'A'}')) {
    throw StateError('the check character did not catch the typo');
  }
  await _until(
    tester,
    'the typo told',
    () => _shown(find.textContaining('That code isn’t right')),
    seconds: 5,
  );
  await _shot(tester, 'a1b-typo');
  // A well-formed code that is not the one shown: tried once, nothing sent, and
  // Fogata comes back online here.
  await type(_otherCode(code));
  await _until(
    tester,
    'the wrong code told',
    () => _shown(find.textContaining('didn’t accept that code')),
    seconds: 240,
  );
  await _shot(tester, 'a1c-wrong-code');
  await _until(
    tester,
    'back online',
    () => state.started && !state.offlineForMove,
    seconds: 60,
  );
  await type(code);
  for (var i = 0; i < 4; i++) {
    await _wait(tester, const Duration(seconds: 1));
    _log(
      'offline ${state.offlineForMove}, move ${state.accountMove?.step} '
      '${state.accountMove?.failure} ${state.accountMove?.failureWords}',
    );
  }
  await _shot(tester, 'a2-looking');
  await _until(
    tester,
    'the question',
    () => _shown(find.byKey(const ValueKey('move-confirm'))),
    seconds: 240,
  );
  await _shot(tester, 'a3-confirm');
  await _tap(tester, find.byKey(const ValueKey('move-yes')));
  await _until(
    tester,
    'moved',
    () => _shown(find.byKey(const ValueKey('move-forget'))),
    seconds: 240,
  );
  await _shot(tester, 'a4-moved');
  if (state.moveState?.kind != 'moved') {
    throw StateError('the folder says ${state.moveState?.kind}');
  }
  if (File('${state.dataDir}/identity.key').existsSync()) {
    throw StateError('the identity is still here');
  }
  // What is left goes; the welcome page shows again.
  await _tap(tester, find.byKey(const ValueKey('move-forget')));
  await _until(
    tester,
    'like new',
    () => _shown(find.byKey(const ValueKey('bring-link'))),
  );
  await _shot(tester, 'a5-like-new');
  File('$_dir/a-done').writeAsStringSync('done');
}

/// The new computer's own yes, once the code matched.
Future<void> _take(WidgetTester tester) async {
  await _until(
    tester,
    'the new computer\'s question',
    () => _shown(find.byKey(const ValueKey('move-take'))),
    seconds: 240,
  );
  await _tap(tester, find.byKey(const ValueKey('move-take')));
}

/// The new computer: "Bring your account" on the welcome page.
Future<void> _new(WidgetTester tester) async {
  await app.main();
  await _until(
    tester,
    'welcome',
    () => _shown(find.byKey(const ValueKey('bring-link'))),
  );
  await _shot(tester, 'b1-welcome');
  await continueUnencryptedIfAsked(tester);
  await _tap(tester, find.byKey(const ValueKey('bring-link')));
  await _until(
    tester,
    'the code',
    () => _shown(find.byKey(const ValueKey('move-code'))),
    seconds: 60,
  );
  final code = tester
      .widget<Text>(find.byKey(const ValueKey('move-code')))
      .data!;
  File('$_dir/code.txt').writeAsStringSync(code);
  await _shot(tester, 'b2-code');
  await _until(
    tester,
    'found',
    () => _shown(find.text('Found your other computer')),
    seconds: 240,
  );
  await _shot(tester, 'b3-found');
  await _take(tester);
  await _until(
    tester,
    'welcome back',
    () => _shown(find.text('Welcome back, Ana')),
    seconds: 240,
  );
  await _shot(tester, 'b4-done');
  await _tap(tester, find.byKey(const ValueKey('move-open')));
  await _until(
    tester,
    'the app',
    () => _shown(find.byType(Shell)),
    seconds: 60,
  );
  final state = AppScope.read(tester.element(find.byType(Shell)));
  await _until(
    tester,
    'Ana and her group',
    () =>
        state.contacts.profile?.person.name == 'Ana' &&
        state.contacts.groups.any((g) => g.name == 'The usual crew'),
    seconds: 60,
  );
  await _shot(tester, 'b5-app');
  if (_shown(find.byType(BringCard))) throw StateError('the card stayed');
  // Ana's computer is done with it too.
  await _until(
    tester,
    'a done',
    () => File('$_dir/a-done').existsSync(),
    seconds: 120,
  );
}

/// Ana's computer with a group, Settings → Move account, the code typed and confirmed.
Future<AppState> _anaMoves(WidgetTester tester, String codeFile) async {
  await app.main();
  await _until(tester, 'welcome', () => _shown(find.byType(TextField)));
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      'Ana';
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => _shown(find.byType(Shell)));
  final state = AppScope.read(tester.element(find.byType(Shell)));
  await state.createGroup('The usual crew', 2, const []);
  await _until(tester, 'the group', () => state.contacts.groups.isNotEmpty);
  File('$_dir/a-key.txt').writeAsStringSync(state.contacts.profile!.person.id);
  state.openSettings(SettingsSection.move);
  final file = File('$_dir/$codeFile');
  await _until(tester, 'the code from b', file.existsSync, seconds: 300);
  final field = find.descendant(
    of: find.byKey(const ValueKey('move-code-field')),
    matching: find.byType(TextField),
  );
  await _until(tester, 'the move page', () => _shown(field));
  tester.widget<TextField>(field).controller!.text = file
      .readAsStringSync()
      .trim();
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.byKey(const ValueKey('move-continue')));
  await _until(
    tester,
    'the question',
    () => _shown(find.byKey(const ValueKey('move-confirm'))),
    seconds: 240,
  );
  await _tap(tester, find.byKey(const ValueKey('move-yes')));
  return state;
}

/// The old computer in the replace, lost and keep scenarios.
Future<void> _oldThen(WidgetTester tester) async {
  final state = await _anaMoves(tester, 'code.txt');
  if (_scenario == 'replace') {
    await _until(
      tester,
      'moved',
      () => _shown(find.byKey(const ValueKey('move-forget'))),
      seconds: 240,
    );
    await _shot(tester, 'a-moved');
    return;
  }
  // The last message is lost: this computer waits for the new one to confirm.
  await _until(
    tester,
    'waiting to confirm',
    () => _shown(find.text('Finish the move')),
    seconds: 240,
  );
  await _shot(tester, 'a-unconfirmed');
  if (state.moveState?.kind != 'moving') {
    throw StateError('the folder says ${state.moveState?.kind}');
  }
  if (_scenario == 'lost') {
    final file = File('$_dir/code2.txt');
    await _until(
      tester,
      'the code that finishes',
      file.existsSync,
      seconds: 300,
    );
    final field = find.descendant(
      of: find.byKey(const ValueKey('move-finish-field')),
      matching: find.byType(TextField),
    );
    tester.widget<TextField>(field).controller!.text = file
        .readAsStringSync()
        .trim();
    await _wait(tester, const Duration(milliseconds: 300));
    await _tap(tester, find.byKey(const ValueKey('move-finish')));
    // Finishing asks again, naming the computer, as a first move does.
    await _until(
      tester,
      'the question again',
      () => _shown(find.byKey(const ValueKey('move-confirm'))),
      seconds: 240,
    );
    await _shot(tester, 'a-finish-question');
    await _tap(tester, find.byKey(const ValueKey('move-yes')));
    await _until(
      tester,
      'moved',
      () => _shown(find.byKey(const ValueKey('move-forget'))),
      seconds: 240,
    );
    await _shot(tester, 'a-finished');
    if (File('${state.dataDir}/identity.key').existsSync()) {
      throw StateError('the identity is still here');
    }
    return;
  }
  // keep: once the new computer threw it away, this one keeps using the account.
  await _until(
    tester,
    'b threw it away',
    () => File('$_dir/b-discarded').existsSync(),
    seconds: 300,
  );
  await _tap(tester, find.textContaining('keep using this one'));
  await _shot(tester, 'a-keep-warning');
  await _tap(tester, find.text('Keep using it here'));
  await _until(tester, 'the app again', () => _shown(find.byType(Shell)));
  final again = AppScope.read(tester.element(find.byType(Shell)));
  await _until(
    tester,
    'Ana and her group again',
    () =>
        again.contacts.profile?.person.name == 'Ana' &&
        again.contacts.groups.isNotEmpty,
    seconds: 60,
  );
  await _shot(tester, 'a-kept');
}

/// The new computer has a profile of its own, Leo, and brings Ana's in its place.
Future<void> _replacing(WidgetTester tester) async {
  await app.main();
  await _until(tester, 'welcome', () => _shown(find.byType(TextField)));
  await continueUnencryptedIfAsked(tester);
  tester.widget<TextField>(find.byType(TextField).first).controller!.text =
      'Leo';
  await _wait(tester, const Duration(milliseconds: 300));
  await _tap(tester, find.text('Create'));
  await _until(tester, 'home', () => _shown(find.byType(Shell)));
  final state = AppScope.read(tester.element(find.byType(Shell)));
  state.openSettings(SettingsSection.move);
  await _tap(tester, find.byKey(const ValueKey('move-bring')));
  await _shot(tester, 'b-replace-warning');
  await _tap(tester, find.byKey(const ValueKey('move-replace')));
  await _until(
    tester,
    'the code',
    () => _shown(find.byKey(const ValueKey('move-code'))),
    seconds: 60,
  );
  final code = tester
      .widget<Text>(find.byKey(const ValueKey('move-code')))
      .data!;
  File('$_dir/code.txt').writeAsStringSync(code);
  await _shot(tester, 'b-replace-code');
  await _take(tester);
  await _until(
    tester,
    'welcome back',
    () => _shown(find.text('Welcome back, Ana')),
    seconds: 240,
  );
  await _shot(tester, 'b-replace-done');
  // Opening starts Fogata again as Ana (see _replaced); this copy quits.
  File('$_dir/b-result.txt').writeAsStringSync('passed\n');
  await _tap(tester, find.byKey(const ValueKey('move-open')));
  await _wait(tester, const Duration(seconds: 60));
  throw StateError('still running after opening the account');
}

/// The copy started again after the account replaced Leo: Ana with her group.
Future<void> _replaced(WidgetTester tester) async {
  File('$_dir/b-restarted.pid').writeAsStringSync('$pid');
  await app.main();
  await _until(
    tester,
    'the app',
    () => _shown(find.byType(Shell)),
    seconds: 60,
  );
  final state = AppScope.read(tester.element(find.byType(Shell)));
  final key = File('$_dir/a-key.txt').readAsStringSync();
  await _until(
    tester,
    'Ana and her group',
    () =>
        state.contacts.profile?.person.id == key &&
        state.contacts.groups.any((g) => g.name == 'The usual crew'),
    seconds: 60,
  );
  await _shot(tester, 'b-replaced-app');
}

/// The new computer when the last message is lost: finishes with a new code (lost), or
/// throws the account away (keep).
Future<void> _newLosing(WidgetTester tester) async {
  await app.main();
  await continueUnencryptedIfAsked(tester);
  await _tap(tester, find.byKey(const ValueKey('bring-link')));
  await _until(
    tester,
    'the code',
    () => _shown(find.byKey(const ValueKey('move-code'))),
    seconds: 60,
  );
  final code = tester
      .widget<Text>(find.byKey(const ValueKey('move-code')))
      .data!;
  File('$_dir/code.txt').writeAsStringSync(code);
  await _take(tester);
  await _until(
    tester,
    'the end unconfirmed',
    () => _shown(find.text('Almost there')),
    seconds: 240,
  );
  await _shot(tester, 'b-unconfirmed');
  final state = AppScope.read(tester.element(find.byType(BringCard)));
  if (_scenario == 'keep') {
    await _tap(tester, find.text('Make a new profile instead'));
    await _tap(tester, find.text('Throw it away'));
    await _until(
      tester,
      'the welcome page again',
      () => _shown(find.byKey(const ValueKey('bring-link'))),
      seconds: 60,
    );
    if (Directory('${state.dataDir}/incoming').existsSync()) {
      throw StateError('what came is still here');
    }
    File('$_dir/b-discarded').writeAsStringSync('done');
    await _shot(tester, 'b-discarded');
    return;
  }
  await _tap(tester, find.byKey(const ValueKey('move-try-again')));
  await _until(
    tester,
    'a code that finishes',
    () =>
        _shown(find.byKey(const ValueKey('move-code'))) &&
        tester.widget<Text>(find.byKey(const ValueKey('move-code'))).data !=
            code,
    seconds: 60,
  );
  await _shot(tester, 'b-finish-code');
  File('$_dir/code2.txt').writeAsStringSync(
    tester.widget<Text>(find.byKey(const ValueKey('move-code'))).data!,
  );
  await _until(
    tester,
    'welcome back',
    () => _shown(find.text('Welcome back, Ana')),
    seconds: 240,
  );
  await _shot(tester, 'b-finished');
  await _tap(tester, find.byKey(const ValueKey('move-open')));
  await _until(
    tester,
    'the app',
    () => _shown(find.byType(Shell)),
    seconds: 60,
  );
  final app2 = AppScope.read(tester.element(find.byType(Shell)));
  await _until(
    tester,
    'Ana and her group',
    () =>
        app2.contacts.profile?.person.name == 'Ana' &&
        app2.contacts.groups.isNotEmpty,
    seconds: 60,
  );
  await _shot(tester, 'b-app');
}
