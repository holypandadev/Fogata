import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'while connecting, each person’s step keeps to its line in the smallest window',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 4)..callState.call = null;
      final long = [
        for (final (i, name) in [
          'Bartolomé Wolfgang Hohenzollern-Sigmarin',
          'Ana María de los Ángeles Villanueva Ruiz',
          'Diego Alejandro Montenegro Castellanos',
        ].indexed)
          UiPerson(
            id: 'p$i',
            name: name,
            short: 'AB${i}C·D${i}EF',
            avatar: UiAvatar(preset: i),
          ),
      ];
      final g = state.contacts.groups.single;
      final summary = UiRoomSummary(
        startedMs: g.room!.startedMs,
        participants: [me, ...long],
        joined: true,
      );
      state
        ..contacts.groups = [
          UiGroup(
            id: g.id,
            name: g.name,
            localName: '',
            icon: 0,
            members: 4,
            online: 3,
            manager: true,
            rank: 0,
            outranks: const [],
            maxMembers: 20,
            maxInRoom: 8,
            voiceRooms: [
              UiVoiceRoom(id: AppState.firstRoom, name: '', room: summary),
            ],
            room: summary,
            openInvites: 0,
            needsUpdate: false,
          ),
        ]
        ..roomState.room = UiRoom(
          startedMs: summary.startedMs,
          joined: true,
          participants: const [
            UiParticipant(person: me, away: false, me: true),
          ],
          chat: const [],
          files: const [],
        )
        ..roomState.memories['g'] = (RoomMemory(summary.startedMs)
          ..joinedMs = DateTime.now().millisecondsSinceEpoch
          ..othersExpected = true);
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 600));
      expect(tester.takeException(), isNull);
      expect(find.text('Connecting to the room'), findsOneWidget);
      Finder inCard(String text) => find.descendant(
        of: find.byWidgetPredicate(
          (w) => w.runtimeType.toString() == '_Connecting',
        ),
        matching: find.text(text),
      );
      for (final p in long) {
        final name = tester.getRect(inCard(p.name));
        expect(name.height, lessThan(24), reason: p.name);
      }
      final steps = inCard('looking for the best route…');
      expect(steps, findsNWidgets(3));
      for (final e in steps.evaluate()) {
        expect(tester.getSize(find.byWidget(e.widget)).height, lessThan(24));
      }
      // Leave nothing ticking.
      await tester.pumpWidget(const SizedBox());
    },
  );
}
