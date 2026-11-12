// The members page fits the narrowest page the window gives it, with text up to 1.5 times
// its size: long names, and members reached through a member with a long name.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/members.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart' show loadFonts;

const _me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

UiPerson _person(String id, String name) => UiPerson(
  id: id,
  name: name,
  short: 'AB${id}C·D${id}EF',
  avatar: const UiAvatar(preset: 2),
);

const _carrier = 'Sofía Fernández de la Peña Altamirano';

void main() {
  testWidgets('member rows fit, however long the way a member comes', (
    tester,
  ) async {
    await loadFonts();
    final state = AppState('/nonexistent/fogata-members-fit-test')
      ..contacts.profile = const UiProfile(
        person: _me,
        fingerprint: '7Q4M K2XR',
      )
      ..contacts.groups = [
        const UiGroup(
          id: 'g',
          name: 'Friday games',
          localName: '',
          icon: 0,
          members: 4,
          online: 2,
          manager: false,
          rank: 3,
          outranks: [],
          managers: 2,
          maxMembers: 20,
          maxInRoom: 8,
          voiceRooms: [UiVoiceRoom(id: AppState.firstRoom, name: '')],
          openInvites: 0,
          needsUpdate: false,
        ),
      ]
      ..nav.membersGroup = 'g'
      ..nav.page = Screen.members;
    state.contacts.members['g'] = [
      UiMember(
        away: false,
        person: _person('1', _carrier),
        rank: 0,
        manager: true,
        me: false,
        path: UiPath.lan,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
      UiMember(
        away: false,
        person: _person('2', 'Ana'),
        rank: 1,
        manager: true,
        me: false,
        path: UiPath.throughMember,
        through: '1',
        waitingForKey: true,
        fingerprint: 'AB2C D2EF 9PLD 3VHC',
      ),
      UiMember(
        away: false,
        person: _person('3', 'Leo Bartolomé Castellanos de Urquijo'),
        rank: 2,
        manager: false,
        me: false,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
      const UiMember(
        away: false,
        person: _me,
        rank: 3,
        manager: false,
        me: true,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
    ];
    // The page at its narrowest (a 960 px window, beside the rail and the side column)
    // and a little wider.
    for (final width in [552.0, 648.0]) {
      for (final scale in [1.0, 1.5]) {
        tester.view.physicalSize = Size(width, 900);
        tester.view.devicePixelRatio = 1;
        tester.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(tester.view.reset);
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        await tester.pumpWidget(
          AppScope(
            state: state,
            child: MaterialApp(
              theme: Fg.darkTokens.themeData,
              home: const Material(child: MembersPage()),
            ),
          ),
        );
        await tester.pump(const Duration(milliseconds: 400));
        expect(tester.takeException(), isNull, reason: '$width at $scale×');
        expect(
          find.textContaining('through $_carrier'),
          findsOneWidget,
          reason: '$width at $scale×',
        );
      }
    }
    // A member's short fingerprint tells their whole one when hovered, not itself again.
    expect(find.byTooltip('Fingerprint AB2C D2EF 9PLD 3VHC'), findsOneWidget);
    expect(find.byTooltip('AB2C·D2EF'), findsNothing);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
