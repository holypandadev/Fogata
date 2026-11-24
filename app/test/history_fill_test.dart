// A group whose history nears its ceiling says so in its settings, from three quarters on;
// its first member is offered the move to a fresh history, behind a confirmation, and
// nobody else is. A group the window shows that moved stays shown under its new id.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/group_settings.dart';
import 'package:fogata/src/theme.dart';

UiGroup _group({String id = 'g', double? fill, bool mayMove = false}) =>
    UiGroup(
      id: id,
      name: 'Sundays',
      localName: '',
      icon: 0,
      members: 3,
      online: 0,
      manager: mayMove,
      rank: mayMove ? 0 : 2,
      managers: 1,
      outranks: const [],
      maxMembers: 20,
      maxInRoom: 8,
      voiceRooms: const [UiVoiceRoom(id: AppState.firstRoom, name: '')],
      openInvites: 0,
      needsUpdate: false,
      raiseAnyway: false,
      fill: fill,
      mayMove: mayMove,
    );

/// Ana's settings of the group, on its members' tab (the one every member sees).
AppState _settings({double? fill, bool mayMove = false}) =>
    AppState('/nonexistent/fogata-history-fill-test')
      ..contacts.groups = [_group(fill: fill, mayMove: mayMove)]
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 2;

Future<void> _show(WidgetTester tester, AppState s) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: s,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Stack(children: [GroupSettingsLayer()])),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  const nearly = 'This group’s history is nearly full.';
  const full =
      'This group’s history is full: until it moves to a fresh one, only '
      'removing members, leaving and the move still work.';
  const hint =
      'Move the group to a fresh history: everyone keeps the group, its name '
      'and what it holds.';
  const firstMoves = 'Its first member can move it to a fresh history.';
  final move = find.byKey(const ValueKey('move-group'));

  for (final fill in [null, 0.5, 0.74]) {
    testWidgets('nothing is said with the history at $fill', (tester) async {
      await _show(tester, _settings(fill: fill, mayMove: true));
      expect(tester.takeException(), isNull);
      expect(find.byKey(const ValueKey('history-fill')), findsNothing);
      expect(move, findsNothing);
    });
  }

  testWidgets('from three quarters, the first member is told and may move it', (
    tester,
  ) async {
    await _show(tester, _settings(fill: 0.75, mayMove: true));
    expect(tester.takeException(), isNull);
    expect(find.text(nearly), findsOneWidget);
    expect(find.text(hint), findsOneWidget);
    expect(move, findsOneWidget);
  });

  testWidgets('full, the first member is told what still works', (
    tester,
  ) async {
    await _show(tester, _settings(fill: 1.0, mayMove: true));
    expect(find.text(full), findsOneWidget);
    expect(find.text(nearly), findsNothing);
    expect(move, findsOneWidget);
  });

  for (final fill in [0.75, 1.0]) {
    testWidgets('a member who may not move it is told, with no button, at '
        '$fill', (tester) async {
      await _show(tester, _settings(fill: fill));
      expect(find.byKey(const ValueKey('history-fill')), findsOneWidget);
      expect(find.text(firstMoves), findsOneWidget);
      expect(find.text(hint), findsNothing);
      expect(move, findsNothing);
    });
  }

  testWidgets('the move asks first, and cancelling does nothing', (
    tester,
  ) async {
    final s = _settings(fill: 0.9, mayMove: true);
    await _show(tester, s);
    await tester.tap(move);
    await tester.pumpAndSettle();
    expect(find.text('Move this group to a fresh history?'), findsOneWidget);
    expect(
      find.textContaining('members away for more than 30 days'),
      findsOneWidget,
    );
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(find.text('Move this group to a fresh history?'), findsNothing);
    expect(s.nav.groupSettings, 'g');
  });

  test('a group shown that moved stays shown under its new id', () {
    final s = AppState('/nonexistent/fogata-history-fill-follow')
      ..contacts.groups = [_group(id: 'new')]
      ..nav.selectedGroup = 'old'
      ..nav.membersGroup = 'old'
      ..nav.groupSettings = 'old'
      ..nav.page = Screen.members;
    s.followGroup('old', 'new');
    expect(s.nav.selectedGroup, 'new');
    expect(s.nav.membersGroup, 'new');
    expect(s.nav.groupSettings, 'new');
    expect(s.nav.page, Screen.members);
  });

  test('another group shown is left as it was', () {
    final s = AppState('/nonexistent/fogata-history-fill-other')
      ..contacts.groups = [_group(id: 'new'), _group(id: 'other')]
      ..nav.selectedGroup = 'other'
      ..nav.groupSettings = 'other';
    s.followGroup('old', 'new');
    expect(s.nav.selectedGroup, 'other');
    expect(s.nav.groupSettings, 'other');
  });
}
