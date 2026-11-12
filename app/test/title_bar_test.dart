import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/host.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';
import 'package:fogata/src/window.dart';

import 'fake_room.dart';

Future<AppState> _show(
  WidgetTester tester,
  AppState state, {
  Size size = const Size(1280, 800),
}) async {
  // The window's buttons ask the window manager, which tests do not have.
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  // A fresh tree, so a state shown before is read again.
  await tester.pumpWidget(const SizedBox());
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 400));
  return state;
}

AppState _home() => fakeRoom(people: 2)
  ..callState.call = null
  ..roomState.room = null
  ..roomState.roomGroup = null
  ..nav.selectedGroup = null
  ..nav.page = Screen.home;

void main() {
  setUpAll(loadFonts);

  testWidgets('the title bar shows no picture of the person', (tester) async {
    for (final state in [_home(), fakeRoom(people: 3)]) {
      await _show(tester, state);
      final bar = find.byType(TitleBar);
      expect(
        find.descendant(of: bar, matching: find.byType(Avatar)),
        findsNothing,
      );
      // Their panel opens from their pill at the bottom left.
      expect(find.byKey(const ValueKey('own-pill')), findsOneWidget);
      // Never started, so nothing to stop.
      state.callState.call = null;
    }
  });

  testWidgets(
    'Home searches with the round button, its shortcut in its tooltip',
    (tester) async {
      await _show(tester, _home());
      final find_ = find.byKey(const ValueKey('find'));
      expect(find_, findsOneWidget);
      expect(tester.getSize(find_), const Size(28, 28));
      expect(find.textContaining('Go to a chat'), findsNothing);
      expect(
        find.byTooltip(
          RegExp(r'^Go to a chat, group or voice room \((Ctrl K|⌘K)\)$'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'the bar has its own window buttons, except on macOS: room for the traffic lights',
    (tester) async {
      final linux = await _show(tester, _home());
      final bar = find.byType(TitleBar);
      final buttons = find.descendant(
        of: bar,
        matching: find.byType(WindowButtons),
      );
      expect(buttons, findsOneWidget);
      final row = find.descendant(of: bar, matching: find.byType(Row)).first;
      final left = tester.getTopLeft(row).dx;
      linux.callState.call = null;

      pinnedHost = HostSystem.macos;
      addTearDown(() => pinnedHost = HostSystem.linux);
      final mac = await _show(tester, _home());
      expect(buttons, findsNothing);
      expect(tester.getTopLeft(row).dx, left + 70);
      expect(
        find.byTooltip(RegExp(r'^Go to a chat, group or voice room \(⌘K\)$')),
        findsOneWidget,
      );
      mac.callState.call = null;
    },
  );

  testWidgets('the group and voice room show whole in a window 866 wide', (
    tester,
  ) async {
    final state = fakeRoom(people: 1);
    final g = state.contacts.groups.single;
    state.contacts.groups = [
      UiGroup(
        id: 'g',
        name: 'Group for game nights',
        localName: '',
        icon: 0,
        members: 2,
        online: 0,
        manager: true,
        rank: 0,
        outranks: const [],
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: [
          g.voiceRooms.first,
          const UiVoiceRoom(id: 'v2', name: 'Elden Ring'),
          const UiVoiceRoom(id: 'v3', name: 'Call of Duty'),
        ],
        room: g.room,
        openInvites: 0,
        needsUpdate: false,
        raiseAnyway: false,
      ),
    ];
    for (final (voice, name) in [
      (AppState.firstRoom, 'General'),
      ('v3', 'Call of Duty'),
    ]) {
      state.roomState.roomVoice = voice;
      await _show(tester, state, size: const Size(866, 548));
      final bar = find.byType(TitleBar);
      for (final text in ['Group for game nights', name]) {
        final paragraph = tester.renderObject<RenderParagraph>(
          find.descendant(of: bar, matching: find.text(text)),
        );
        expect(paragraph.didExceedMaxLines, isFalse, reason: text);
      }
    }
    // Never started, so nothing to stop.
    state.callState.call = null;
  });
}
