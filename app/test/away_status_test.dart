// Away: the moon instead of the green dot, and "Away" in words, wherever a friend's or a
// member's presence shows, and on this person's own pill.

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/user_panel.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _show(WidgetTester tester, AppState state) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
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
  expect(tester.takeException(), isNull);
}

/// The pages' timers (link countdowns, tooltips) end with them.
Future<void> _end(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

Finder _dots(Finder within, {required bool away}) => find.descendant(
  of: within,
  matching: find.byWidgetPredicate(
    (w) => w is StatusDot && w.away == away && !w.offline,
  ),
);

UiFriend _friend(int i, UiPresence presence) => UiFriend(
  person: other(i),
  fingerprint: 'AB0C D0EF 9PLD 3VH$i',
  presence: presence,
  delivered: true,
);

AppState _friends() {
  final state = fakeRoom(people: 2)
    ..callState.call = null
    ..roomState.room = null
    ..nav.selectedGroup = null
    ..nav.page = Screen.friends
    // In no room, so the pill says the status.
    ..contacts.groups = []
    ..contacts.friends = [
      _friend(0, UiPresence.online),
      _friend(1, UiPresence.away),
      _friend(2, UiPresence.offline),
    ];
  state.direct.take([
    for (final (i, away) in [false, true].indexed)
      UiDirectChat(
        friend: other(i),
        online: true,
        away: away,
        last: 'hello',
        lastMine: false,
        lastMs: 1000 - i,
        unread: 0,
        timerMs: 0,
      ),
  ]);
  return state;
}

void main() {
  setUpAll(loadFonts);

  testWidgets('an away friend shows a moon and "Away", and counts as online', (
    tester,
  ) async {
    await _show(tester, _friends());
    final online = find.byKey(const ValueKey('friend-row:p0'));
    final away = find.byKey(const ValueKey('friend-row:p1'));
    expect(_dots(online, away: false), findsOneWidget);
    expect(_dots(away, away: true), findsOneWidget);
    expect(
      find.descendant(of: away, matching: find.text('Away')),
      findsOneWidget,
    );
    // Online and away count in the Online tab; the offline one does not.
    expect(find.text('2'), findsWidgets);
    // The direct chats beside show the moon too.
    expect(
      find.byWidgetPredicate((w) => w is StatusDot && w.away && w.inset == 2),
      findsOneWidget,
    );
    await _end(tester);
  });

  testWidgets('the own pill says Away, and Offline wins over it', (
    tester,
  ) async {
    final state = _friends()..contacts.away = true;
    await _show(tester, state);
    final pill = find.byKey(const ValueKey('own-pill'));
    expect(find.descendant(of: pill, matching: find.text('Away')), findsOne);
    expect(_dots(pill, away: true), findsOneWidget);
    final text = tester.widget<Text>(
      find.descendant(of: pill, matching: find.text('Away')),
    );
    expect(text.style?.color, Fg.darkTokens.amber);

    state.prefs.appearOffline = true;
    state.prefs.changed();
    await tester.pump(const Duration(milliseconds: 400));
    expect(
      find.descendant(of: pill, matching: find.text('Away')),
      findsNothing,
    );
    expect(find.descendant(of: pill, matching: find.text('Offline')), findsOne);
    expect(_dots(pill, away: true), findsNothing);
    await _end(tester);
  });

  testWidgets('an away member says so in amber', (tester) async {
    final state = fakeRoom(people: 2)
      ..callState.call = null
      ..roomState.room = null
      ..nav.page = Screen.members
      ..nav.membersGroup = 'g';
    state.contacts.members['g'] = [
      UiMember(
        person: me,
        rank: 0,
        manager: true,
        me: true,
        away: false,
        waitingForKey: false,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
      for (final (i, away) in [false, true].indexed)
        UiMember(
          person: other(i),
          rank: i + 1,
          manager: false,
          me: false,
          path: UiPath.direct,
          away: away,
          waitingForKey: false,
          fingerprint: 'AB0C D0EF 9PLD 3VH$i',
        ),
    ];
    await _show(tester, state);
    final away = find.text('away · direct, encrypted');
    expect(away, findsOneWidget);
    expect(tester.widget<Text>(away).style?.color, Fg.darkTokens.amber);
    expect(find.text('online · direct, encrypted'), findsOneWidget);
    await _end(tester);
  });

  testWidgets('the moon is a disc with a bite of the background', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Center(
          child: RepaintBoundary(
            key: ValueKey('dot'),
            child: StatusDot(offline: false, away: true, size: 40),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 400));
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(const ValueKey('dot')),
    );
    final image = await tester.runAsync(() => boundary.toImage());
    final bytes = (await tester.runAsync(() => image!.toByteData()))!;
    Color at(int x, int y) {
      final o = (y * image!.width + x) * 4;
      return Color.fromARGB(
        bytes.getUint8(o + 3),
        bytes.getUint8(o),
        bytes.getUint8(o + 1),
        bytes.getUint8(o + 2),
      );
    }

    // Amber at the bottom right, the background at the top left of the disc.
    expect(at(28, 28), Fg.darkTokens.amber);
    expect(at(10, 10), Fg.darkTokens.bg2);
  });
}
