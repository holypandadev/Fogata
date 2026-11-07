import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/home.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart' show fakeRoom, loadFonts, me, other;

UiGroup _live(String id, int people) {
  final room = UiRoomSummary(
    startedMs: DateTime.now().millisecondsSinceEpoch - 60000,
    participants: [for (var i = 0; i < people; i++) other(i)],
    joined: false,
  );
  return UiGroup(
    id: id,
    name: 'Group $id',
    localName: '',
    icon: 0,
    members: people + 1,
    online: people,
    manager: false,
    rank: 1,
    outranks: const [],
    maxMembers: 20,
    maxInRoom: 8,
    voiceRooms: [UiVoiceRoom(id: AppState.firstRoom, name: '', room: room)],
    room: room,
    openInvites: 0,
  );
}

void main() {
  setUpAll(loadFonts);

  testWidgets('live rooms side by side are as tall as the tallest', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1000, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-live-cards-test')
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..contacts.groups = [_live('a', 5), _live('b', 2), _live('c', 1)];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Home()),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 400));
    expect(tester.takeException(), isNull);
    final cards = find.byType(Panel);
    // Two in the first row, the third alone below.
    final a = tester.getRect(cards.at(0));
    final b = tester.getRect(cards.at(1));
    final c = tester.getRect(cards.at(2));
    expect(b.top, a.top);
    expect(b.height, a.height);
    expect(c.top, greaterThan(a.bottom));
    expect(c.height, lessThan(a.height));
    // The buttons sit on the same line at the bottom of both.
    final joins = find.text('Join');
    expect(tester.getRect(joins.at(0)).top, tester.getRect(joins.at(1)).top);
  });

  testWidgets('nothing on Home moves while rooms are live', (tester) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = fakeRoom(people: 1)
      ..contacts.groups = [_live('a', 3), _live('b', 2)]
      ..callState.call = null
      ..roomState.room = null
      ..roomState.roomGroup = null
      ..nav.selectedGroup = null
      ..nav.page = Screen.home;
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
    tester.takeException();
    // Live on the cards and in the side column, and no dot pulsing for it.
    expect(find.widgetWithText(Pill, 'Live'), findsNWidgets(2));
    expect(find.byType(SlowPulse), findsNothing);
    expect(find.byType(Dot), findsNothing);
    await tester.pump(const Duration(seconds: 1));
    expect(tester.binding.hasScheduledFrame, isFalse);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
