// A group waiting to turn on its stronger protection tells those who manage it who has not
// updated Fogata yet, in its settings; nobody else is told, and a group not waiting says
// nothing. A group that went up past this version tells every member, in its settings and
// in the bell, for as long as the group says so.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/bell.dart';
import 'package:fogata/src/screens/group_settings.dart';
import 'package:fogata/src/theme.dart';

const _ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);
const _bea = UiPerson(
  id: 'bea',
  name: 'Bea',
  short: 'AB1C·D2EF',
  avatar: UiAvatar(preset: 2),
);
const _leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB3C·D4EF',
  avatar: UiAvatar(preset: 3),
);

/// Ana's settings of a group with Bea and Leo, on its members' tab.
AppState _settings({
  required bool manager,
  List<UiPerson>? behind,
  bool needsUpdate = false,
}) => AppState('/nonexistent/fogata-level-waiting-test')
  ..contacts.profile = const UiProfile(person: _ana, fingerprint: '7Q4M K2XR')
  ..contacts.groups = [
    UiGroup(
      id: 'g',
      name: 'Sundays',
      localName: '',
      icon: 0,
      members: 3,
      online: 0,
      manager: manager,
      rank: manager ? 0 : 2,
      managers: manager ? null : 1,
      outranks: const [],
      maxMembers: 20,
      maxInRoom: 8,
      voiceRooms: const [UiVoiceRoom(id: AppState.firstRoom, name: '')],
      openInvites: 0,
      levelBehind: behind,
      needsUpdate: needsUpdate,
    ),
  ]
  ..contacts.members['g'] = [
    for (final (i, p) in [_ana, _bea, _leo].indexed)
      UiMember(
        away: false,
        person: p,
        rank: i,
        manager: true,
        me: p == _ana,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
  ]
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
  const waiting =
      'This group is waiting to turn on stronger protection. It turns on by '
      'itself once everyone has updated Fogata. Still to update: Bea, Leo';

  testWidgets('a manager sees who has to update before the group goes up', (
    tester,
  ) async {
    await _show(tester, _settings(manager: true, behind: [_bea, _leo]));
    expect(tester.takeException(), isNull);
    expect(find.text(waiting), findsOneWidget);
  });

  testWidgets('nothing is said when nobody holds the group back', (
    tester,
  ) async {
    await _show(tester, _settings(manager: true));
    expect(find.textContaining('stronger protection'), findsNothing);
  });

  testWidgets('a member who does not manage the group is not told', (
    tester,
  ) async {
    await _show(tester, _settings(manager: false, behind: [_bea, _leo]));
    expect(find.textContaining('stronger protection'), findsNothing);
  });

  for (final manager in [false, true]) {
    testWidgets('a group gone past this version says so to '
        '${manager ? 'a manager' : 'a member'}', (tester) async {
      final s = _settings(manager: manager, needsUpdate: true);
      await _show(tester, s);
      expect(tester.takeException(), isNull);
      expect(find.byKey(const ValueKey('group-update-note')), findsOneWidget);
      expect(find.text('This group needs a newer Fogata'), findsOneWidget);
      // No version ready yet: nothing to restart into.
      expect(find.byKey(const ValueKey('group-update-restart')), findsNothing);
    });
  }

  testWidgets('a group that does not need a newer Fogata says nothing of it', (
    tester,
  ) async {
    await _show(tester, _settings(manager: false));
    expect(find.byKey(const ValueKey('group-update-note')), findsNothing);
  });

  test('the bell tells of a group gone past this version while it is', () {
    final s = _settings(manager: false, needsUpdate: true);
    final notice = s.notices.list.single;
    expect(notice.kind, NoticeKind.groupUpdate);
    expect(notice.group?.id, 'g');
    // Updated: the group no longer says so, and neither does the bell.
    s.contacts.groups = [
      for (final g in s.contacts.groups)
        UiGroup(
          id: g.id,
          name: g.name,
          localName: g.localName,
          icon: g.icon,
          members: g.members,
          online: g.online,
          manager: g.manager,
          rank: g.rank,
          managers: g.managers,
          outranks: g.outranks,
          maxMembers: g.maxMembers,
          maxInRoom: g.maxInRoom,
          voiceRooms: g.voiceRooms,
          openInvites: g.openInvites,
          needsUpdate: false,
        ),
    ];
    expect(s.notices.list, isEmpty);
  });

  testWidgets('the bell names the group that needs a newer Fogata', (
    tester,
  ) async {
    final s = _settings(manager: false, needsUpdate: true)
      ..notices.bellFrom = const Rect.fromLTWH(1200, 10, 28, 28);
    await _show(tester, s);
    await tester.pumpWidget(
      AppScope(
        state: s,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Stack(children: [BellLayer()])),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('“Sundays” needs a newer Fogata'), findsOneWidget);
  });
}
