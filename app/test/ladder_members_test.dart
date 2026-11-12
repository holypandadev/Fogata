// The members of a group in their order: the marks of each place, the menu only on those
// below this device, leaving as the first, and who manages the group in its settings.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/format.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/group_settings.dart';
import 'package:fogata/src/screens/members.dart';
import 'package:fogata/src/theme.dart';

const people = [
  UiPerson(
    id: 'ana',
    name: 'Ana',
    short: '7Q4M·K2XR',
    avatar: UiAvatar(preset: 1),
  ),
  UiPerson(
    id: 'bea',
    name: 'Bea',
    short: 'AB1C·D2EF',
    avatar: UiAvatar(preset: 2),
  ),
  UiPerson(
    id: 'leo',
    name: 'Leo',
    short: 'AB3C·D4EF',
    avatar: UiAvatar(preset: 3),
  ),
  UiPerson(
    id: 'sofia',
    name: 'Sofía',
    short: 'CD5E·F6GH',
    avatar: UiAvatar(preset: 4),
  ),
];

/// Ana, Bea, Leo and Sofía in that order; this device is the one at [mine]. With
/// [managers], only the first that many manage.
AppState ladder(int mine, {int? managers}) {
  final manages = managers == null || mine < managers;
  return AppState('/nonexistent/fogata-ladder-test')
    ..contacts.profile = UiProfile(
      person: people[mine],
      fingerprint: '7Q4M K2XR',
    )
    ..contacts.groups = [
      UiGroup(
        id: 'g',
        name: 'Sundays',
        localName: '',
        icon: 0,
        members: people.length,
        online: 0,
        manager: manages,
        rank: mine,
        managers: managers,
        outranks: [for (final p in people.skip(mine + 1)) p.id],
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: const [UiVoiceRoom(id: AppState.firstRoom, name: '')],
        openInvites: 0,
        needsUpdate: false,
        keptOut: const [],
      ),
    ]
    ..nav.membersGroup = 'g'
    ..nav.page = Screen.members
    ..contacts.members['g'] = [
      // Shuffled: the page puts them in order.
      for (final i in [2, 0, 3, 1])
        UiMember(
          away: false,
          person: people[i],
          rank: i,
          manager: managers == null || i < managers,
          me: i == mine,
          waitingForKey: false,
          fingerprint: '7Q4M K2XR 9PLD 3VHC',
        ),
    ];
}

Widget app(AppState s, Widget child) => AppScope(
  state: s,
  child: MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Material(child: child),
  ),
);

Future<void> show(WidgetTester tester, AppState s, Widget child) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(app(s, child));
  await tester.pump(const Duration(milliseconds: 400));
}

void main() {
  test('places read as people say them', () {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 103].map(ordinal).toList(), [
      '1st',
      '2nd',
      '3rd',
      '4th',
      '11th',
      '12th',
      '13th',
      '21st',
      '22nd',
      '103rd',
    ]);
  });

  testWidgets('the list goes in order, the first marked, the others by place', (
    tester,
  ) async {
    await show(tester, ladder(1), const MembersPage());
    final tops = [
      for (final name in ['Ana', 'Bea (you)', 'Leo', 'Sofía'])
        tester.getTopLeft(find.text(name)).dy,
    ];
    expect([...tops]..sort(), tops);
    expect(find.text('First'), findsOneWidget);
    expect(find.text('2nd'), findsOneWidget);
    expect(find.text('3rd'), findsOneWidget);
    expect(find.text('4th'), findsOneWidget);
    expect(find.text('In order: each one acts on those below'), findsOneWidget);
    // Options only for those below: Leo and Sofía.
    expect(find.byTooltip('Options for Leo'), findsOneWidget);
    expect(find.byTooltip('Options for Sofía'), findsOneWidget);
    expect(find.byTooltip('Options for Ana'), findsNothing);
    expect(find.byTooltip('Options for Bea'), findsNothing);
    // Everyone manages: anyone can leave, and invite.
    expect(find.text('Leave group'), findsOneWidget);
    expect(find.text('Copy invite link'), findsOneWidget);
  });

  testWidgets(
    'the menu moves up only someone not just below, down only before the end',
    (tester) async {
      final s = ladder(1);
      await show(tester, s, const MembersPage());
      await tester.tap(find.byTooltip('Options for Leo'));
      await tester.pumpAndSettle();
      // Leo is just below Bea: no move up; he is not last: he can go to the end.
      expect(find.text('Move up below me'), findsNothing);
      expect(find.text('Move to the end'), findsOneWidget);
      expect(find.text('Time out of a room…'), findsOneWidget);
      expect(find.text('Remove from the group'), findsOneWidget);
      await tester.tapAt(const Offset(5, 5));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Options for Sofía'));
      await tester.pumpAndSettle();
      expect(find.text('Move up below me'), findsOneWidget);
      expect(find.text('Move to the end'), findsNothing);
      // A time out asks how long.
      await tester.tap(find.text('Time out of a room…'));
      await tester.pumpAndSettle();
      expect(find.text('Time Sofía out'), findsOneWidget);
      expect(find.text('5 minutes'), findsOneWidget);
      expect(find.text('Until let back'), findsOneWidget);
    },
  );

  testWidgets('the last one acts on nobody', (tester) async {
    await show(tester, ladder(3), const MembersPage());
    expect(find.byTooltip(RegExp('Options for')), findsNothing);
    expect(find.text('Sofía (you)'), findsOneWidget);
  });

  testWidgets('those who do not manage are told who does', (tester) async {
    await show(tester, ladder(2, managers: 2), const MembersPage());
    expect(find.text('Only the first 2 can change the group.'), findsOneWidget);
    expect(find.text('Copy invite link'), findsNothing);
    expect(find.text('Group settings'), findsNothing);
    // Leo still acts on Sofía.
    expect(find.byTooltip('Options for Sofía'), findsOneWidget);
  });

  testWidgets('the first may leave, and is told who becomes first', (
    tester,
  ) async {
    await show(tester, ladder(0), const MembersPage());
    await tester.tap(find.text('Leave group'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Bea becomes first.'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    // Anyone else leaves without that line.
    await show(tester, ladder(2), const MembersPage());
    await tester.tap(find.text('Leave group'));
    await tester.pumpAndSettle();
    expect(find.textContaining('becomes first'), findsNothing);
  });

  testWidgets(
    'the settings choose who manages, never leaving out the one choosing',
    (tester) async {
      final s = ladder(1)
        ..nav.groupSettings = 'g'
        ..nav.groupSettingsTab = 2;
      await show(tester, s, const Stack(children: [GroupSettingsLayer()]));
      await tester.pumpAndSettle();
      expect(find.text('2nd in this group'), findsOneWidget);
      expect(find.text('Who manages this group'), findsOneWidget);
      expect(find.text('Everyone'), findsOneWidget);
      // The fewest to choose is this device's own place.
      expect(find.text('The first 2'), findsOneWidget);
      expect(find.byType(Slider), findsNothing);
      // Up, end, time out and remove only for those below.
      expect(find.text('Move up'), findsOneWidget);
      expect(find.text('To the end'), findsOneWidget);
      expect(find.byTooltip('Remove from the group'), findsNWidgets(2));

      final closed = ladder(1, managers: 3)
        ..nav.groupSettings = 'g'
        ..nav.groupSettingsTab = 2;
      await show(tester, closed, const Stack(children: [GroupSettingsLayer()]));
      await tester.pumpAndSettle();
      expect(find.text('The first 3'), findsOneWidget);
      expect(find.byType(Slider), findsOneWidget);
      expect(
        find.textContaining('nobody can manage the group any more'),
        findsOneWidget,
      );
    },
  );

  testWidgets('the first N go up to those in the group now, never past them', (
    tester,
  ) async {
    Slider slider() => tester.widget<Slider>(find.byType(Slider));
    // Four members, this device first, the first two manage: from 1 to 4.
    final four = ladder(0, managers: 2)
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 2;
    await show(tester, four, const Stack(children: [GroupSettingsLayer()]));
    await tester.pumpAndSettle();
    expect((slider().min, slider().max, slider().value), (1.0, 4.0, 2.0));
    expect(slider().divisions, 3);
    expect(find.textContaining('Only the first 2 change'), findsOneWidget);
    // One alone: said as one.
    final one = ladder(0, managers: 1)
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 2;
    await show(tester, one, const Stack(children: [GroupSettingsLayer()]));
    await tester.pumpAndSettle();
    expect(find.textContaining('Only the first one changes'), findsOneWidget);
    // Set to more than the members now (the group shrank): shown at the members, said so.
    final shrank = ladder(0, managers: 6)
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 2;
    await show(tester, shrank, const Stack(children: [GroupSettingsLayer()]));
    await tester.pumpAndSettle();
    expect(slider().value, 4.0);
    expect(find.text('The first 4'), findsOneWidget);
    expect(
      find.text(
        'Set to the first 6, more than the 4 in the group now: for now that is everyone.',
      ),
      findsOneWidget,
    );
    // Alone in the group: the first one is everyone, and cannot be chosen.
    final alone = AppState('/nonexistent/fogata-ladder-alone')
      ..contacts.profile = UiProfile(
        person: people[0],
        fingerprint: '7Q4M K2XR',
      )
      ..contacts.groups = [
        const UiGroup(
          id: 'g',
          name: 'Sundays',
          localName: '',
          icon: 0,
          members: 1,
          online: 0,
          manager: true,
          rank: 0,
          outranks: [],
          maxMembers: 20,
          maxInRoom: 8,
          voiceRooms: [UiVoiceRoom(id: AppState.firstRoom, name: '')],
          openInvites: 0,
          needsUpdate: false,
          keptOut: [],
        ),
      ]
      ..contacts.members['g'] = [
        UiMember(
          away: false,
          person: people[0],
          rank: 0,
          manager: true,
          me: true,
          waitingForKey: false,
          fingerprint: '7Q4M K2XR 9PLD 3VHC',
        ),
      ]
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 2;
    await show(tester, alone, const Stack(children: [GroupSettingsLayer()]));
    await tester.pumpAndSettle();
    final choice = find.ancestor(
      of: find.text('The first one'),
      matching: find.byType(Opacity),
    );
    expect(tester.widget<Opacity>(choice.first).opacity, .5);
    await tester.tap(find.text('The first one'));
    await tester.pumpAndSettle();
    expect(find.byType(Slider), findsNothing);
    expect(alone.contacts.groups.first.managers, isNull);
  });
}
