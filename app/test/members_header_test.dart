import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/members.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart' show loadFonts;

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

const name = 'Sunday football, padel and drinks with the whole neighbourhood';

void main() {
  testWidgets(
    'a manager’s buttons leave the group’s name room to show, down to the smallest window',
    (tester) async {
      await loadFonts();
      final state = AppState('/nonexistent/fogata-members-header-test')
        ..contacts.profile = const UiProfile(
          person: me,
          fingerprint: '7Q4M K2XR',
        )
        ..contacts.groups = [
          const UiGroup(
            id: 'g',
            name: name,
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
          ),
        ]
        ..nav.membersGroup = 'g'
        ..nav.page = Screen.members;
      state.contacts.members['g'] = const [
        UiMember(
          away: false,
          person: me,
          rank: 0,
          manager: true,
          me: true,
          waitingForKey: false,
          fingerprint: '7Q4M K2XR 9PLD 3VHC',
        ),
      ];
      // The page beside the rail and the side column: from the smallest window to a
      // wide one.
      for (final width in [648.0, 968.0, 1608.0]) {
        tester.view.physicalSize = Size(width, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
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
        expect(tester.takeException(), isNull);
        final title = tester.getSize(find.text(name));
        expect(title.width, greaterThanOrEqualTo(360), reason: 'at $width');
        // One line: the name is shortened, never broken letter by letter.
        expect(title.height, lessThan(48), reason: 'at $width');
        expect(find.text('Copy invite link'), findsOneWidget);
        expect(find.text('Back to the room'), findsNothing);
        expect(find.text('Open room'), findsOneWidget);
      }
    },
  );
}
