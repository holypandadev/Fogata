import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/group_settings.dart';
import 'package:fogata/src/theme.dart';

const ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);
const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

UiMember member(UiPerson p, int rank) => UiMember(
  away: false,
  person: p,
  rank: rank,
  manager: true,
  me: p == ana,
  waitingForKey: false,
  fingerprint: '7Q4M K2XR 9PLD 3VHC',
);

void main() {
  testWidgets('a friend whose invitation was declined can be invited again', (
    tester,
  ) async {
    const group = UiGroup(
      id: 'g',
      name: 'Call',
      localName: '',
      icon: 0,
      members: 1,
      online: 0,
      manager: true,
      rank: 0,
      maxMembers: 20,
      maxInRoom: 8,
      voiceRooms: [],
      openInvites: 0,
      needsUpdate: false,
      outranks: [],
    );
    final state = AppState('/nonexistent/fogata-invite-friends-test')
      ..contacts.profile = const UiProfile(
        person: ana,
        fingerprint: '7Q4M K2XR',
      )
      ..contacts.groups = [group]
      ..contacts.friends = [
        const UiFriend(
          person: sofia,
          fingerprint: 'C07N 50S4',
          presence: UiPresence.online,
          delivered: true,
        ),
      ]
      ..contacts.members['g'] = [member(ana, 0)];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Scaffold(body: InviteFriends(group: group)),
        ),
      ),
    );
    expect(find.text('Sofía'), findsOneWidget);
    expect(find.text('Invite'), findsOneWidget);
    // Invited: on the group's list until she joins or declines.
    state.contacts.members['g'] = [member(ana, 0), member(sofia, 1)];
    state.notifyListeners();
    await tester.pump();
    expect(find.text('Sofía'), findsNothing);
    expect(find.text('All your friends are in the group.'), findsOneWidget);
    // Declined: she left the list, and can be invited again.
    state.contacts.members['g'] = [member(ana, 0)];
    state.notifyListeners();
    await tester.pump();
    expect(find.text('Invite'), findsOneWidget);
    expect(find.text('Invited'), findsNothing);
  });
}
