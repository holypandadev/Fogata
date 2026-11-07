import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/room_count.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _show(WidgetTester tester, AppState state, {Fg? theme}) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: (theme ?? Fg.darkTokens).themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await _settle(tester);
}

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Ends a test: the pages' notes (the core is not there in tests) go by themselves.
Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

/// The fake room's group (this device and [people] - 1 others in General) with two more
/// voice rooms: Gaming with [gaming] people, and Quiet, empty; at most [max] in each.
AppState _rooms({int people = 3, int gaming = 2, int max = 8}) {
  final state = fakeRoom(people: people);
  _limit(state, max, gaming: gaming);
  return state;
}

/// Gives the group a new most per room, as a change in its log does.
void _limit(AppState state, int max, {int gaming = 2}) {
  final g = state.contacts.groups.first;
  state.contacts.groups = [
    UiGroup(
      id: g.id,
      name: g.name,
      localName: '',
      icon: 0,
      members: 20,
      online: g.online,
      manager: true,
      rank: 0,
      outranks: const [],
      maxMembers: 20,
      maxInRoom: max,
      voiceRooms: [
        g.voiceRooms.first,
        UiVoiceRoom(
          id: 'v2',
          name: 'Gaming',
          room: UiRoomSummary(
            startedMs: 0,
            participants: [for (var i = 10; i < 10 + gaming; i++) other(i)],
            joined: false,
          ),
        ),
        const UiVoiceRoom(id: 'v3', name: 'Quiet'),
      ],
      room: g.room,
      openInvites: 0,
    ),
  ];
}

/// The counts shown, by what they say to a screen reader (within a button's label).
Finder _count(String label) =>
    find.bySemanticsLabel(RegExp(RegExp.escape(label)));

/// Every count shown: people, most, and whether it is the header's pill.
Set<(int, int, bool)> _counts(WidgetTester tester) => {
  for (final w in tester.widgetList<RoomCount>(find.byType(RoomCount)))
    (w.count, w.max, w.pill),
};

void main() {
  setUpAll(loadFonts);

  setUp(() {
    // The window's buttons ask the window manager, which tests do not have.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets('each voice room in the column, and the header, say how full', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final state = _rooms();
    await _show(tester, state);
    expect(_counts(tester), {
      (3, 8, false),
      (2, 8, false),
      (0, 8, false),
      (3, 8, true),
    });
    // The column's, and the header's.
    expect(find.text('3/8', findRichText: true), findsNWidgets(2));
    expect(find.text('2/8', findRichText: true), findsOneWidget);
    expect(find.text('0/8', findRichText: true), findsOneWidget);
    expect(_count('2 of 8 people'), findsOneWidget);
    expect(find.textContaining('Full', findRichText: true), findsNothing);
    handle.dispose();
    await _done(tester);
  });

  testWidgets('a full room reads as full, and a new limit shows at once', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final state = _rooms();
    await _show(tester, state);
    // A manager lowers the limit to 3: General (this device's) and its header are full.
    _limit(state, 3);
    state.go(state.nav.page);
    await _settle(tester);
    expect(_counts(tester), {
      (3, 3, false),
      (2, 3, false),
      (0, 3, false),
      (3, 3, true),
    });
    // The header says it in words.
    expect(find.text('Full · '), findsOneWidget);
    // The room this device is in only turns amber beside "you are in"; another full room
    // says so.
    expect(find.text('3/3', findRichText: true), findsNWidgets(2));
    _limit(state, 2);
    state.go(state.nav.page);
    await _settle(tester);
    expect(find.text('Full 2/2', findRichText: true), findsOneWidget);
    expect(_count('Full, 2 of 2 people'), findsOneWidget);
    expect(find.text('3/2', findRichText: true), findsNWidgets(2));
    expect(tester.takeException(), isNull);
    handle.dispose();
    await _done(tester);
  });

  testWidgets('home’s live card lists people under their voice room', (
    tester,
  ) async {
    final state = _rooms(gaming: 8)..goHome();
    await _show(tester, state, theme: Fg.lightTokens);
    expect(find.text('General'), findsOneWidget);
    expect(find.text('Gaming'), findsOneWidget);
    expect(find.text('3/8', findRichText: true), findsOneWidget);
    expect(find.text('Full 8/8', findRichText: true), findsOneWidget);
    // Gaming's people come after its name.
    final gaming = tester.getTopLeft(find.text('Gaming')).dy;
    expect(
      tester.getTopLeft(find.text(other(10).name).first).dy,
      greaterThan(gaming),
    );
    expect(tester.takeException(), isNull);
    await _done(tester);
  });

  testWidgets(
    'group settings: the rooms count, and the limit follows a change',
    (tester) async {
      final state = _rooms();
      await _show(tester, state);
      state.openGroupSettings('g', tab: 1);
      await _settle(tester);
      expect(find.text('3/8', findRichText: true), findsWidgets);
      expect(find.text('2/8', findRichText: true), findsWidgets);
      state.openGroupSettings('g', tab: 4);
      await _settle(tester);
      double room() => tester.widget<Slider>(find.byType(Slider).last).value;
      expect(room(), 8);
      // Another manager sets 5: the slider shows it without reopening.
      _limit(state, 5);
      state.go(state.nav.page);
      await _settle(tester);
      expect(room(), 5);
      expect(tester.takeException(), isNull);
      await _done(tester);
    },
  );

  testWidgets('a long voice room name keeps the column’s labels on one line', (
    tester,
  ) async {
    final state = fakeRoom(people: 3);
    final g = state.contacts.groups.first;
    final long = 'A voice room called by a name forty long';
    state.contacts.groups = [
      UiGroup(
        id: g.id,
        name: g.name,
        localName: '',
        icon: 0,
        members: 20,
        online: g.online,
        manager: true,
        rank: 0,
        outranks: const [],
        maxMembers: 20,
        maxInRoom: 8,
        voiceRooms: [
          UiVoiceRoom(id: AppState.firstRoom, name: long, room: g.room),
        ],
        room: g.room,
        openInvites: 0,
      ),
    ];
    await _show(tester, state);
    final label = find.text(long);
    expect(label, findsOneWidget);
    // One line of 13-point text, the words after the name whole beside it.
    expect(tester.getSize(label).height, lessThan(24));
    expect(find.text(' · you are in'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await _done(tester);
  });
}
