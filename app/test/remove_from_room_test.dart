// The admin's "Remove from <room>" in the person menu asks first and shows as what it is.

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/volume.dart';

const ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);
const bea = UiPerson(
  id: 'bea',
  name: 'Bea',
  short: 'AB1C·D2EF',
  avatar: UiAvatar(preset: 2),
);

UiMember member(UiPerson p, {int rank = 1, bool me = false}) => UiMember(
  away: false,
  person: p,
  rank: rank,
  manager: true,
  me: me,
  waitingForKey: false,
  fingerprint: '7Q4M K2XR 9PLD 3VHC',
);

void main() {
  testWidgets('removing someone from a room asks first, in red', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1280, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final state = AppState('/nonexistent/fogata-remove-from-room')
      ..contacts.profile = const UiProfile(
        person: ana,
        fingerprint: '7Q4M K2XR',
      )
      ..contacts.groups = [
        const UiGroup(
          id: 'g',
          name: 'Call',
          localName: '',
          icon: 0,
          members: 2,
          online: 2,
          manager: true,
          rank: 0,
          maxMembers: 20,
          maxInRoom: 8,
          voiceRooms: [UiVoiceRoom(id: AppState.firstRoom, name: 'General')],
          openInvites: 0,
          needsUpdate: false,
          outranks: ['bea'],
        ),
      ]
      ..contacts.members['g'] = [member(ana, rank: 0, me: true), member(bea)];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Scaffold(
            body: Center(
              child: VolumeTarget(
                person: bea,
                group: 'g',
                voice: AppState.firstRoom,
                child: SizedBox(width: 200, height: 100, child: Text('Bea')),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Bea'), buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    final row = find.text('Remove from General');
    expect(tester.widget<Text>(row).style?.color, Fg.darkTokens.red);
    await tester.tap(row);
    await tester.pumpAndSettle();
    expect(find.text('Remove Bea from General?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(find.text('Remove Bea from General?'), findsNothing);
    expect(state.toasts.list, isEmpty, reason: 'nothing was asked of the core');

    await tester.tap(find.text('Bea'), buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove from General'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove'));
    await tester.pumpAndSettle();
    // There is no core here: the removal was asked for, and failed.
    expect(state.toasts.list.where((t) => t.error), hasLength(1));
    await tester.pump(const Duration(seconds: 8));
  });
}
