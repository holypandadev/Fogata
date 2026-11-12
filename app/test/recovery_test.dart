import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/recovery.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/recovery.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'wait.dart';

import 'fake_room.dart';

const _key =
    'ABCD EFGH JKMN PQRS TVWX YZ01 2345 6789 ABCD EFGH JKMN PQRS TVWX Y';

/// The core's recovery copies as the screens call them, and what they asked of it, in
/// order.
class FakeCore implements RustLibApi {
  final calls = <String>[];
  bool on = false;
  bool replacedKept = false;
  String? turnedOnWith;
  bool forgotKey = false;
  final exports = <(String, bool)>[];
  final restores = <(bool, bool)>[];

  /// What the restores answer, in turn.
  final outcomes = <UiRestoreOutcome>[];
  UiCopyOpened opened = _opened;
  UiAccountHere? here;

  /// Turning on fails this many times first.
  int turnOnFails = 0;

  /// Making a copy, or deleting the replaced account, fails.
  bool fails = false;

  /// Making a copy waits for this, when set.
  Completer<void>? exportWaits;

  /// As at the start of a test.
  void reset() {
    calls.clear();
    on = false;
    replacedKept = false;
    turnedOnWith = null;
    forgotKey = false;
    exports.clear();
    restores.clear();
    outcomes.clear();
    opened = _opened;
    here = null;
    turnOnFails = 0;
    fails = false;
    exportWaits = null;
  }

  static const _opened = UiCopyOpened(
    summary: UiCopySummary(
      madeOn: '2026-11-01',
      name: 'Marta',
      fingerprint: '7Q4M K2XR 9PLD 3VHC',
      files: 12,
      bytes: 3400000,
      attachments: false,
    ),
  );

  @override
  Future<UiRecoveryState> crateApiRecoveryRecoveryState({
    required String dataDir,
  }) async => UiRecoveryState(
    isOn: on,
    keyId: on ? '7Q4M K2XR' : null,
    replacedKept: replacedKept,
  );

  @override
  UiNewRecoveryKey crateApiRecoveryRecoveryNewKey() {
    calls.add('new key');
    return const UiNewRecoveryKey(text: _key, id: '7Q4M K2XR');
  }

  @override
  void crateApiRecoveryRecoveryForgetNewKey() => forgotKey = true;

  @override
  Future<void> crateApiRecoveryRecoveryTurnOn({
    required String dataDir,
    required String id,
  }) async {
    if (turnOnFails > 0) {
      turnOnFails--;
      throw AnyhowException('cannot write recovery.bin');
    }
    turnedOnWith = id;
    on = true;
  }

  @override
  UiKeyProblem? crateApiRecoveryRecoveryKeyProblem({required String text}) {
    final plain = text.replaceAll(RegExp(r'[\s-]'), '').toUpperCase();
    if (plain.length != 53) return UiKeyProblem.notAKey;
    return plain == _key.replaceAll(' ', '') ? null : UiKeyProblem.mistyped;
  }

  @override
  String crateApiRecoveryRecoveryCopyName() => 'Fogata-2026-11-05.fogata-copy';

  @override
  bool crateApiRecoveryRecoveryInDataFolder({
    required String dataDir,
    required String path,
  }) => path.startsWith(dataDir);

  @override
  Future<PlatformInt64> crateApiRecoveryRecoveryAttachmentsBytes({
    required String dataDir,
  }) async => 48200000;

  @override
  Future<UiExported> crateApiRecoveryRecoveryExport({
    required String dataDir,
    required String to,
    required bool attachments,
  }) async {
    calls.add('export');
    await exportWaits?.future;
    if (fails) throw AnyhowException('the disk went away');
    exports.add((to, attachments));
    return const UiExported(files: 40, bytes: 51000000);
  }

  @override
  Future<bool> crateApiRecoveryRecoveryForgetReplaced({
    required String dataDir,
  }) async {
    calls.add('forget');
    if (fails) throw AnyhowException('cannot remove replaced');
    replacedKept = false;
    return true;
  }

  @override
  Future<UiCopyInfo> crateApiRecoveryRecoveryInspect({
    required String dataDir,
    required String copy,
  }) async => copy.endsWith('.fogata-copy')
      ? const UiCopyInfo(madeOn: '2026-11-01', thisKey: false)
      : const UiCopyInfo(madeOn: '', problem: UiCopyProblem.notACopy);

  @override
  Future<UiCopyOpened> crateApiRecoveryRecoveryOpen({
    required String copy,
    required String key,
  }) async => opened;

  @override
  Future<UiAccountHere?> crateApiRecoveryRecoveryAccountIn({
    required String dataDir,
  }) async => here;

  @override
  Future<UiRestoreOutcome> crateApiRecoveryRecoveryRestore({
    required String dataDir,
    required String copy,
    required String key,
    required bool replace,
    required bool unencrypted,
  }) async {
    restores.add((replace, unencrypted));
    return outcomes.removeAt(0);
  }

  @override
  Future<void> crateApiAppStopCore({required String dataDir}) async =>
      calls.add('stop');

  @override
  Future<UiProfile> crateApiAppStart({required String dataDir}) async {
    calls.add('start');
    throw StateError('not started in tests');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

Future<AppState> _show(
  WidgetTester tester,
  Widget child,
  AppState state,
) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(
          child: Center(child: SizedBox(width: 640, child: child)),
        ),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
  return state;
}

/// Lets the fake core answer, then the screens follow and their dialogs come or go.
Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// The toasts' texts.
List<String> _said(AppState state) =>
    state.toasts.list.map((t) => t.text).toList();

Future<void> _end(WidgetTester tester, AppState state) async {
  state.toasts.dispose();
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 2));
}

void main() {
  setUpAll(() async {
    // Measured with the interface's fonts, as people see it.
    await loadFonts();
    RustLib.initMock(api: core);
  });
  setUp(core.reset);

  testWidgets('copies are turned on only once two groups of the key are typed '
      'back', (tester) async {
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1),
    );
    expect(find.text('Off'), findsOneWidget);
    await tester.tap(find.text('Turn on…'));
    await _settle(tester);
    // The key, shown once, with the warning.
    expect(find.text(_key), findsOneWidget);
    expect(find.textContaining('Fogata doesn’t keep this key'), findsOneWidget);
    await tester.tap(find.text('I wrote it down'));
    await _settle(tester);

    // The groups asked, in order, as their labels say.
    final asked = [
      for (final w in tester.widgetList<Text>(
        find.textContaining(RegExp(r'^Group \d+$')),
      ))
        int.parse(w.data!.split(' ').last) - 1,
    ];
    expect(asked, hasLength(2));
    expect(asked[0], lessThan(asked[1]));
    // Never the last group, which is only the check character.
    final groups = _key.split(' ');
    expect(asked.last, lessThan(groups.length - 1));

    // A wrong group is said, and the key is not taken.
    await tester.enterText(
      find.byKey(const ValueKey('recovery-group-0')),
      groups[asked[0]] == 'ABCD' ? 'EFGH' : 'ABCD',
    );
    await tester.pump();
    expect(
      find.text('That isn’t it: look again at what you wrote down.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Turn on copies'));
    await _settle(tester);
    expect(core.turnedOnWith, isNull);

    // Both right, in small letters: taken.
    await tester.enterText(
      find.byKey(const ValueKey('recovery-group-0')),
      groups[asked[0]].toLowerCase(),
    );
    await tester.enterText(
      find.byKey(const ValueKey('recovery-group-1')),
      groups[asked[1]],
    );
    await tester.pump();
    await tester.tap(find.text('Turn on copies'));
    await _settle(tester);
    expect(core.turnedOnWith, '7Q4M K2XR');
    expect(core.forgotKey, isFalse);
    expect(find.byKey(const ValueKey('recovery-check')), findsNothing);
    expect(_said(state), contains('Copies are on, for the key 7Q4M K2XR.'));
    expect(find.text('On · key 7Q4M K2XR'), findsOneWidget);
    await _end(tester, state);
  });

  testWidgets('a key shown and not taken goes', (tester) async {
    core.on = true;
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1),
    );
    // Changing the key warns about older copies first.
    await tester.tap(find.text('Change key…'));
    await _settle(tester);
    expect(
      find.textContaining('Copies made before still open'),
      findsOneWidget,
    );
    await tester.tap(find.text('Make a new key'));
    await _settle(tester);
    expect(find.text(_key), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await _settle(tester);
    expect(core.forgotKey, isTrue);
    expect(core.turnedOnWith, isNull);
    await _end(tester, state);
  });

  testWidgets('the new key is blurred until the eye button shows it, stays shown, and has no '
      'copy button', (tester) async {
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1),
    );
    await tester.tap(find.text('Turn on…'));
    await _settle(tester);
    final blurred = find.ancestor(
      of: find.text(_key),
      matching: find.byType(ImageFiltered),
    );
    // Blurred as a friend link is, so a shared screen shows nothing of it.
    expect(blurred, findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('recovery-new-key')),
        matching: find.byType(CopyButton),
      ),
      findsNothing,
    );
    await tester.tap(find.byTooltip('Show key'));
    await tester.pump();
    expect(blurred, findsNothing);
    // It stays for as long as writing it down takes, unlike a link or a code, and is
    // blurred again with the same button.
    await tester.pump(SecretBox.shownFor * 3);
    await tester.pump();
    expect(blurred, findsNothing);
    await tester.tap(find.byTooltip('Hide'));
    await tester.pump();
    expect(blurred, findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await _settle(tester);
    await _end(tester, state);
  });

  testWidgets('a copy is made with the core stopped, never in the data '
      'folder', (tester) async {
    core.on = true;
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1)..started = true,
    );
    final places = [
      '${state.dataDir}/Fogata.fogata-copy',
      '/backup/F.fogata-copy',
    ];
    final offered = <String>[];
    chooseWhereToSave = (name, folder) async {
      offered.add(name);
      return places.removeAt(0);
    };
    await tester.tap(find.text('Make a copy…'));
    await _settle(tester);
    // The files sent in chats are off, with what they would add.
    expect(
      find.text('Pictures, videos and files: 48.2 MB more.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Include files sent in chats'));
    await tester.pump();
    await tester.tap(find.text('Choose where…'));
    await _settle(tester);
    expect(
      find.text(
        'A copy can’t be kept in Fogata’s data folder. Choose another place.',
      ),
      findsOneWidget,
    );
    expect(core.calls, isNot(contains('stop')));
    await tester.tap(find.text('Choose where…'));
    await _settle(tester);
    expect(offered, [
      'Fogata-2026-11-05.fogata-copy',
      'Fogata-2026-11-05.fogata-copy',
    ]);
    expect(core.exports, [('/backup/F.fogata-copy', true)]);
    expect(core.calls, ['stop', 'export', 'start']);
    expect(_said(state), contains('Copy saved: F.fogata-copy (51.0 MB).'));
    await _end(tester, state);
  });

  testWidgets('the account a restore replaced is deleted once confirmed', (
    tester,
  ) async {
    core
      ..on = true
      ..replacedKept = true;
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1)..started = true,
    );
    expect(
      find.text(
        'An account that a restore replaced is still kept on this computer.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Delete it…'));
    await _settle(tester);
    await tester.tap(find.text('Cancel'));
    await _settle(tester);
    expect(core.calls, isEmpty);
    await tester.tap(find.text('Delete it…'));
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('recovery-confirm')));
    await _settle(tester);
    expect(core.calls, ['stop', 'forget', 'start']);
    expect(_said(state), contains('The replaced account was deleted.'));
    expect(find.text('Delete it…'), findsNothing);
    await _end(tester, state);
  });

  testWidgets('a copy is restored after its key, with the warning and what it '
      'replaces, and unencrypted when the person chooses so', (tester) async {
    core
      ..here = const UiAccountHere(fingerprint: 'AAAA BBBB CCCC DDDD')
      ..outcomes.addAll([
        const UiRestoreOutcome(
          refused: UiRestoreRefusal.noKeychain,
          need: 0,
          free: 0,
        ),
        const UiRestoreOutcome(
          restored: UiRestored(name: 'Marta', files: 12, replaced: true),
          need: 0,
          free: 0,
        ),
      ]);
    final state = fakeRoom(people: 1)
      ..hasProfile = false
      ..restoring = true;
    await _show(tester, const RestoreCard(), state);
    chooseCopyToOpen = () async => '/backup/Fogata-2026-11-01.fogata-copy';
    await tester.tap(find.text('Choose the copy…'));
    await _settle(tester);
    expect(find.textContaining('A copy made on'), findsOneWidget);
    expect(
      find.textContaining(
        'a different recovery key from the one this computer has',
      ),
      findsOneWidget,
    );

    // A key with a wrong character is said as soon as it is whole.
    final field = find.byKey(const ValueKey('restore-key-field'));
    await tester.enterText(field, 'ABCD EFGH');
    await tester.pump();
    expect(find.byKey(const ValueKey('restore-key-problem')), findsNothing);
    await tester.enterText(field, _key.replaceFirst('A', 'B'));
    await tester.pump();
    expect(
      find.text('A letter or digit is wrong: check them.'),
      findsOneWidget,
    );
    await tester.enterText(field, _key.toLowerCase());
    await tester.pump();
    await tester.tap(find.text('Continue'));
    await _settle(tester);

    // Whose account, the warning, and what is replaced here.
    expect(find.text('Marta’s account'), findsOneWidget);
    expect(find.text('12 files, 3.4 MB in all'), findsOneWidget);
    expect(
      find.textContaining('Use a copy only if no other computer'),
      findsOneWidget,
    );
    expect(
      find.text(
        'This computer holds an account with no profile (device AAAA BBBB CCCC DDDD).',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Replace and restore'));
    await _settle(tester);

    // No keychain: the choice the welcome page offers, then the restore again.
    expect(find.byKey(const ValueKey('restore-keychain')), findsOneWidget);
    await tester.tap(find.text('Continue unencrypted'));
    await _settle(tester);
    // The settings are read again from the disk, as after a move, before the start.
    await pumpUntil(
      tester,
      () => core.calls.contains('start'),
      what: 'the application started',
    );
    await _settle(tester);
    expect(core.restores, [(true, false), (true, true)]);
    expect(state.restoring, isFalse);
    expect(state.hasProfile, isTrue);
    expect(core.calls, contains('start'));
    expect(_said(state), contains('Marta’s account is restored.'));
    await _end(tester, state);
  });

  testWidgets('a copy that does not open is said, and nothing is restored', (
    tester,
  ) async {
    core.opened = const UiCopyOpened(problem: UiCopyProblem.otherKey);
    final state = fakeRoom(people: 1)
      ..hasProfile = false
      ..restoring = true;
    await _show(tester, const RestoreCard(), state);
    chooseCopyToOpen = () async => '/backup/notes.txt';
    await tester.tap(find.text('Choose the copy…'));
    await _settle(tester);
    expect(find.text('This isn’t a copy of a Fogata account.'), findsOneWidget);
    chooseCopyToOpen = () async => '/backup/F.fogata-copy';
    await tester.tap(find.text('Choose the copy…'));
    await _settle(tester);
    await tester.enterText(
      find.byKey(const ValueKey('restore-key-field')),
      _key,
    );
    await tester.pump();
    await tester.tap(find.text('Continue'));
    await _settle(tester);
    expect(
      find.text('This copy was made for another recovery key.'),
      findsOneWidget,
    );
    expect(core.restores, isEmpty);
    // Back to the welcome page's card.
    await tester.tap(find.text('Back'));
    await _settle(tester);
    await tester.tap(find.text('Back'));
    await _settle(tester);
    expect(state.restoring, isFalse);
    await _end(tester, state);
  });

  testWidgets('a copy or a deletion that fails still starts the core again', (
    tester,
  ) async {
    core
      ..on = true
      ..replacedKept = true
      ..fails = true;
    final state = await _show(
      tester,
      const RecoveryCopyPanel(),
      fakeRoom(people: 1)..started = true,
    );
    chooseWhereToSave = (name, folder) async => '/backup/F.fogata-copy';
    await tester.tap(find.text('Make a copy…'));
    await _settle(tester);
    await tester.tap(find.text('Choose where…'));
    await _settle(tester);
    expect(core.calls, ['stop', 'export', 'start']);
    expect(
      _said(state),
      contains('The copy couldn’t be made. The disk went away.'),
    );
    expect(state.stoppedBy, isNull);
    core.calls.clear();
    await tester.tap(find.text('Delete it…'));
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('recovery-confirm')));
    await _settle(tester);
    expect(core.calls, ['stop', 'forget', 'start']);
    expect(state.stoppedBy, isNull);
    // Nothing is left busy: a copy can be made again.
    core.fails = false;
    core.calls.clear();
    await tester.tap(find.text('Make a copy…'));
    await _settle(tester);
    await tester.tap(find.text('Choose where…'));
    await _settle(tester);
    expect(core.calls, ['stop', 'export', 'start']);
    await _end(tester, state);
  });

  testWidgets('while a copy is made, nothing else stops or starts the core', (
    tester,
  ) async {
    core
      ..on = true
      ..replacedKept = true
      ..exportWaits = Completer<void>();
    final state = fakeRoom(people: 1)..started = true;
    // The panel twice: as if Settings were closed and opened again meanwhile.
    await _show(
      tester,
      const Column(
        children: [
          RecoveryCopyPanel(key: ValueKey('first')),
          RecoveryCopyPanel(key: ValueKey('second')),
        ],
      ),
      state,
    );
    chooseWhereToSave = (name, folder) async => '/backup/F.fogata-copy';
    await tester.tap(find.text('Make a copy…').first);
    await _settle(tester);
    await tester.tap(find.text('Choose where…'));
    await _settle(tester);
    expect(core.calls, ['stop', 'export']);
    expect(state.stoppedBy, isNotNull);
    // The other panel's buttons do nothing meanwhile.
    for (final button in [
      'recovery-make',
      'recovery-turn-on',
      'recovery-delete-replaced',
    ]) {
      await tester.tap(find.byKey(ValueKey(button)).last, warnIfMissed: false);
      await _settle(tester);
    }
    expect(find.byKey(const ValueKey('recovery-make-copy')), findsNothing);
    expect(find.byKey(const ValueKey('recovery-confirm')), findsNothing);
    expect(find.byKey(const ValueKey('recovery-new-key')), findsNothing);
    expect(core.calls, ['stop', 'export']);
    // Neither another piece of work nor a move stops it, nor starts it again.
    final other = Object();
    expect(await state.stopForWork(other), isFalse);
    await state.resumeAfterWork(other);
    expect(core.calls, ['stop', 'export']);
    // The copy ends: started once, by the one that stopped it.
    core.exportWaits!.complete();
    await _settle(tester);
    expect(core.calls, ['stop', 'export', 'start']);
    expect(state.stoppedBy, isNull);
    await _end(tester, state);
  });

  testWidgets(
    'turning on that fails says so and keeps the key for another try',
    (tester) async {
      core.turnOnFails = 1;
      final state = await _show(
        tester,
        const RecoveryCopyPanel(),
        fakeRoom(people: 1),
      );
      await tester.tap(find.text('Turn on…'));
      await _settle(tester);
      await tester.tap(find.text('I wrote it down'));
      await _settle(tester);
      final groups = _key.split(' ');
      final asked = [
        for (final w in tester.widgetList<Text>(
          find.textContaining(RegExp(r'^Group \d+$')),
        ))
          int.parse(w.data!.split(' ').last) - 1,
      ];
      await tester.enterText(
        find.byKey(const ValueKey('recovery-group-0')),
        groups[asked[0]],
      );
      await tester.enterText(
        find.byKey(const ValueKey('recovery-group-1')),
        groups[asked[1]],
      );
      await tester.pump();
      await tester.tap(find.text('Turn on copies'));
      await _settle(tester);
      expect(find.text('Cannot write recovery.bin.'), findsOneWidget);
      expect(find.byKey(const ValueKey('recovery-check')), findsOneWidget);
      expect(core.forgotKey, isFalse);
      expect(core.turnedOnWith, isNull);
      await tester.tap(find.text('Turn on copies'));
      await _settle(tester);
      expect(core.turnedOnWith, '7Q4M K2XR');
      expect(core.forgotKey, isFalse);
      await _end(tester, state);
    },
  );

  /// The welcome page's card at the step before restoring, with the key typed.
  Future<AppState> toRestore(WidgetTester tester) async {
    final state = fakeRoom(people: 1)
      ..hasProfile = false
      ..restoring = true;
    await _show(tester, const RestoreCard(), state);
    chooseCopyToOpen = () async => '/backup/F.fogata-copy';
    await tester.tap(find.text('Choose the copy…'));
    await _settle(tester);
    await tester.enterText(
      find.byKey(const ValueKey('restore-key-field')),
      _key,
    );
    await tester.pump();
    await tester.tap(find.text('Continue'));
    await _settle(tester);
    return state;
  }

  const refused = UiRestoreOutcome(need: 0, free: 0);
  UiRestoreOutcome refusal(
    UiRestoreRefusal why, {
    int need = 0,
    int free = 0,
  }) => UiRestoreOutcome(refused: why, need: need, free: free);
  const restored = UiRestoreOutcome(
    restored: UiRestored(name: 'Marta', files: 12, replaced: false),
    need: 0,
    free: 0,
  );

  testWidgets('each refusal is said, and nothing is restored', (tester) async {
    core.outcomes.addAll([
      refusal(UiRestoreRefusal.noRoom, need: 2400000000, free: 800000000),
      refusal(UiRestoreRefusal.accountHere),
      refusal(UiRestoreRefusal.copyInFolder),
      refused,
    ]);
    final state = await toRestore(tester);
    for (final words in [
      'The copy needs 2.4 GB on this disk, and 800 MB are free. Nothing changed.',
      'This computer holds an account now. Choose the copy again to see what it '
          'replaces.',
      'The copy is inside Fogata’s data folder. Move it somewhere else, then choose '
          'it again.',
      'Something went wrong.',
    ]) {
      await tester.tap(find.byKey(const ValueKey('restore-confirm')));
      await _settle(tester);
      expect(find.text(words), findsOneWidget);
      expect(find.byKey(const ValueKey('restore-delete-kept')), findsNothing);
    }
    expect(state.restoring, isTrue);
    expect(core.calls, isNot(contains('start')));
    await _end(tester, state);
  });

  testWidgets('an account kept aside by an earlier restore can be deleted from '
      'the card, and the restore goes on', (tester) async {
    core.outcomes.addAll([refusal(UiRestoreRefusal.replacedKept), restored]);
    final state = await toRestore(tester);
    await tester.tap(find.byKey(const ValueKey('restore-confirm')));
    await _settle(tester);
    expect(
      find.text(
        'This computer still keeps an account that an earlier restore replaced. '
        'Delete it to restore this copy.',
      ),
      findsOneWidget,
    );
    // Asked first: cancelled, nothing goes.
    await tester.tap(find.text('Delete the account kept aside…'));
    await _settle(tester);
    await tester.tap(find.text('Cancel'));
    await _settle(tester);
    expect(core.calls, isEmpty);
    await tester.tap(find.text('Delete the account kept aside…'));
    await _settle(tester);
    await tester.tap(find.byKey(const ValueKey('recovery-confirm')));
    await _settle(tester);
    await pumpUntil(
      tester,
      () => core.calls.contains('start'),
      what: 'the application started',
    );
    await _settle(tester);
    expect(core.calls.first, 'forget');
    expect(core.restores, hasLength(2));
    expect(state.restoring, isFalse);
    expect(core.calls, contains('start'));
    await _end(tester, state);
  });

  testWidgets('without a keychain, "Try again" restores again encrypted', (
    tester,
  ) async {
    core.outcomes.addAll([
      refusal(UiRestoreRefusal.noKeychain),
      refusal(UiRestoreRefusal.noKeychain),
    ]);
    final state = await toRestore(tester);
    await tester.tap(find.byKey(const ValueKey('restore-confirm')));
    await _settle(tester);
    expect(find.byKey(const ValueKey('restore-keychain')), findsOneWidget);
    await tester.tap(find.text('Try again'));
    await _settle(tester);
    // Asked again, as the keychain still keeps nothing; closed, the card stays.
    expect(core.restores, [(false, false), (false, false)]);
    expect(find.byKey(const ValueKey('restore-keychain')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.byKey(const ValueKey('restore-keychain')), findsNothing);
    expect(find.byKey(const ValueKey('restore-confirm')), findsOneWidget);
    expect(state.restoring, isTrue);
    await _end(tester, state);
  });

  testWidgets('the key typed goes from the field when the card is left', (
    tester,
  ) async {
    final state = await toRestore(tester);
    // Back to the key, then out of the card.
    await tester.tap(find.text('Back'));
    await _settle(tester);
    final field = tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(const ValueKey('restore-key-field')),
        matching: find.byType(EditableText),
      ),
    );
    expect(field.controller.text, _key);
    await tester.tap(find.text('Back'));
    await _settle(tester);
    await tester.tap(find.text('Back'));
    await _settle(tester);
    expect(state.restoring, isFalse);
    expect(field.controller.text, isEmpty);
    await _end(tester, state);
  });
}
