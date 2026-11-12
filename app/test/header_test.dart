import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/window.dart';

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

void main() {
  testWidgets(
    'long names leave the title bar whole down to the narrowest window',
    (tester) async {
      // The window's buttons ask the window manager, which tests do not have.
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      final state = AppState('/nonexistent/fogata-header-test')
        ..contacts.profile = const UiProfile(
          person: me,
          fingerprint: '7Q4M K2XR',
        )
        ..contacts.groups = [
          UiGroup(
            id: 'g',
            name: 'A group with a name far too long to fit ' * 3,
            localName: '',
            icon: 0,
            members: 2,
            online: 1,
            manager: false,
            rank: 1,
            outranks: const [],
            maxMembers: 20,
            maxInRoom: 8,
            voiceRooms: [
              const UiVoiceRoom(id: AppState.firstRoom, name: ''),
              UiVoiceRoom(id: 'v2', name: 'A voice room called ' * 4),
            ],
            openInvites: 0,
            needsUpdate: false,
          ),
        ]
        ..roomState.roomGroup = 'g'
        ..roomState.roomVoice = 'v2'
        ..nav.selectedGroup = 'g';
      for (final page in [Screen.room, Screen.members]) {
        for (final width in [960.0, 1180.0, 1400.0]) {
          state.nav.page = page;
          tester.view.physicalSize = Size(width, 640);
          tester.view.devicePixelRatio = 1;
          await tester.pumpWidget(
            AppScope(
              state: state,
              child: MaterialApp(
                theme: Fg.darkTokens.themeData,
                home: const Material(child: Shell()),
              ),
            ),
          );
          await tester.pump();
          // The test font draws every letter as a wide square, so the pages' own texts may
          // overflow here; only the title bar counts.
          tester.takeException();
          final bar = find.byType(TitleBar);
          final rows = find.descendant(of: bar, matching: find.byType(Row));
          for (final row in tester.renderObjectList<RenderFlex>(rows)) {
            var used = 0.0;
            row.visitChildren(
              (child) => used += (child as RenderBox).size.width,
            );
            expect(
              used,
              lessThanOrEqualTo(row.size.width + .5),
              reason: '$width px',
            );
          }
          expect(find.byTooltip('Close'), findsOneWidget);
        }
      }
      tester.view.reset();
    },
  );

  testWidgets(
    'going into another voice room of the group keeps one highlight',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(1280, 720);
      tester.view.devicePixelRatio = 1;
      // In the first voice room, the window already showing the second one.
      final state = AppState('/nonexistent/fogata-header-test')
        ..contacts.profile = const UiProfile(
          person: me,
          fingerprint: '7Q4M K2XR',
        )
        ..contacts.groups = [
          const UiGroup(
            id: 'g',
            name: 'Friends',
            localName: '',
            icon: 0,
            members: 2,
            online: 1,
            manager: false,
            rank: 1,
            outranks: [],
            maxMembers: 20,
            maxInRoom: 8,
            voiceRooms: [
              UiVoiceRoom(
                id: AppState.firstRoom,
                name: '',
                room: UiRoomSummary(
                  startedMs: 1,
                  participants: [me],
                  joined: true,
                ),
              ),
              UiVoiceRoom(id: 'v2', name: 'Games'),
            ],
            openInvites: 0,
            needsUpdate: false,
          ),
        ]
        ..roomState.roomGroup = 'g'
        ..roomState.roomVoice = 'v2'
        ..nav.selectedGroup = 'g'
        ..nav.page = Screen.room;
      // The test font may make the page's texts overflow; only the keys matter here.
      final errors = <String>[];
      final report = FlutterError.onError;
      FlutterError.onError = (details) => errors.add(details.toString());
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await tester.pump();
      FlutterError.onError = report;
      expect(errors.where((e) => e.contains('GlobalKey')), isEmpty);
      tester.view.reset();
    },
  );
}
