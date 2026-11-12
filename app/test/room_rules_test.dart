// Who may enter a voice room: the part of the person menu for those above, the room list for
// someone kept out, the words they are told and the group settings' access and list.

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/format.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/group_settings.dart';
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
const leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB3C·D4EF',
  avatar: UiAvatar(preset: 3),
);

UiMember member(UiPerson p, int rank, {bool me = false}) => UiMember(
  away: false,
  person: p,
  rank: rank,
  manager: true,
  me: me,
  waitingForKey: false,
  fingerprint: '7Q4M K2XR 9PLD 3VHC',
);

/// A group of Ana, Bea and Leo in that order, with a second room only Bea may enter and Leo
/// kept out of General; this device is Ana, first, with `first`, else Leo, last.
AppState state({required bool first, int? until}) {
  final now = DateTime.now().millisecondsSinceEpoch;
  return AppState('/nonexistent/fogata-room-rules-test')
    ..contacts.profile = UiProfile(
      person: first ? ana : leo,
      fingerprint: '7Q4M K2XR 9PLD 3VHC',
    )
    ..contacts.groups = [
      UiGroup(
        id: 'g',
        name: 'Call',
        localName: '',
        icon: 0,
        members: 3,
        online: 2,
        manager: true,
        rank: first ? 0 : 2,
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: [
          UiVoiceRoom(
            id: AppState.firstRoom,
            name: 'General',
            barrier: first
                ? null
                : UiBarrier(
                    kind: until == null
                        ? UiBarrierKind.untilLetBack
                        : UiBarrierKind.timedOut,
                    by: 'ana',
                    sinceMs: now,
                    untilMs: until,
                  ),
          ),
          const UiVoiceRoom(id: 'gaming', name: 'Gaming', only: ['bea']),
        ],
        openInvites: 0,
        needsUpdate: false,
        outranks: first ? const ['bea', 'leo'] : const [],
        keptOut: first
            ? [
                UiKeptOut(
                  voice: AppState.firstRoom,
                  member: leo,
                  by: ana,
                  untilMs: now + 58 * 60 * 1000,
                ),
                const UiKeptOut(voice: 'gaming', member: bea, by: ana),
              ]
            : null,
      ),
    ]
    ..contacts.members['g'] = [
      member(ana, 0, me: first),
      member(bea, 1),
      member(leo, 2, me: !first),
    ];
}

Widget app(AppState s, Widget child) => AppScope(
  state: s,
  child: MaterialApp(
    theme: Fg.darkTokens.themeData,
    home: Scaffold(body: child),
  ),
);

void main() {
  testWidgets(
    'someone above takes a member out of the room from the person menu',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1280, 800));
      await tester.pumpWidget(
        app(
          state(first: true),
          const Center(
            child: VolumeTarget(
              person: bea,
              group: 'g',
              voice: AppState.firstRoom,
              child: SizedBox(width: 200, height: 100, child: Text('Bea')),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Bea'), buttons: kSecondaryButton);
      await tester.pumpAndSettle();
      expect(find.text('BELOW YOU'), findsOneWidget);
      expect(find.text('Remove from General'), findsOneWidget);
      expect(find.text('5 minutes'), findsNothing);
      await tester.tap(find.text('Time out of General'));
      await tester.pumpAndSettle();
      expect(
        find.text('Bea leaves General and cannot come back for:'),
        findsOneWidget,
      );
      for (final (text, _) in timeOuts) {
        expect(find.text(text), findsOneWidget);
      }
      expect(
        find.text('Until someone above them lets them back'),
        findsOneWidget,
      );
    },
  );

  testWidgets('someone not below this device has no part to take them out', (
    tester,
  ) async {
    final s = state(first: true);
    await tester.pumpWidget(
      app(
        s,
        const Center(
          child: VolumeTarget(
            person: ana,
            group: 'g',
            voice: AppState.firstRoom,
            child: SizedBox(width: 200, height: 100, child: Text('Ana')),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Ana'), buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    expect(find.text('Only you hear these changes'), findsOneWidget);
    expect(find.text('BELOW YOU'), findsNothing);
  });

  test('the words for someone kept out', () {
    final now = DateTime.now().millisecondsSinceEpoch;
    final hour = UiBarrier(
      kind: UiBarrierKind.timedOut,
      by: 'ana',
      sinceMs: now,
      untilMs: now + 3600 * 1000,
    );
    expect(
      AppState.takenOutText('Ana', 'General', hour),
      'Ana took you out of General for 1 hour.',
    );
    expect(
      AppState.takenOutText(
        'Ana',
        'General',
        const UiBarrier(kind: UiBarrierKind.removed),
      ),
      'Ana took you out of General.',
    );
    expect(
      AppState.takenOutText(
        'Ana',
        'General',
        const UiBarrier(kind: UiBarrierKind.untilLetBack),
      ),
      'Ana took you out of General until someone above you lets you back.',
    );
    final s = state(first: false, until: now + 58 * 60 * 1000 - 1000);
    expect(
      AppState.barrierText(
        s.contacts.groups.first.voiceRooms.first,
        nowMs: now,
      ),
      'You can come back to General in 58 min.',
    );
    expect(
      AppState.barrierText(
        state(first: false).contacts.groups.first.voiceRooms.first,
      ),
      'General is not for you now.',
    );
    // A room kept from this device is not the one a group opens in.
    expect(s.contacts.voiceToOpen('g'), 'gaming');
    expect(timeLeft(90 * 60 * 1000), '2 h');
    expect(timeLeft(25 * 3600 * 1000), '2 days');
    expect(spanWords(7 * 24 * 3600 * 1000), '1 week');
    expect(spanWords(5 * 60 * 1000), '5 minutes');
  });

  testWidgets('the group settings show who may enter and who is kept out', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1280, 800));
    final s = state(first: true)
      ..nav.groupSettings = 'g'
      ..nav.groupSettingsTab = 1;
    await tester.pumpWidget(
      app(s, const Stack(children: [GroupSettingsLayer()])),
    );
    await tester.pumpAndSettle();
    expect(find.text('Everyone'), findsOneWidget);
    expect(find.text('Only some'), findsOneWidget);
    expect(find.text('Kept out'), findsOneWidget);
    expect(find.text('Until let back'), findsOneWidget);
    expect(find.text('58 min left'), findsOneWidget);
    expect(find.text('Let back'), findsNWidgets(2));
    expect(find.text('Who can enter Gaming'), findsNothing);
    await tester.tap(find.text('Only some'));
    await tester.pumpAndSettle();
    expect(find.text('Who can enter Gaming'), findsOneWidget);
    expect(find.text('Only the members I choose'), findsOneWidget);
    expect(find.text('Ana (you)'), findsOneWidget);
    expect(
      find.text(
        'You and those above you can always enter. Someone left out sees the room with a lock.',
      ),
      findsOneWidget,
    );
  });
}
