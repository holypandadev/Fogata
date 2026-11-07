import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/friends.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/switcher.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart' show loadFonts, me;

const _leo = UiPerson(
  id: 'leo',
  name: 'Leo',
  short: 'AB1C·D1EF',
  avatar: UiAvatar(preset: 2),
);
const _ana = UiPerson(
  id: 'ana',
  name: 'Ana',
  short: 'AB2C·D2EF',
  avatar: UiAvatar(preset: 3),
);
const _bea = UiPerson(
  id: 'bea',
  name: 'Bea',
  short: 'AB3C·D3EF',
  avatar: UiAvatar(preset: 4),
);

UiFriend _friend(UiPerson p) => UiFriend(
  person: p,
  fingerprint: p.short,
  presence: UiPresence.online,
  delivered: true,
);

UiDirectChat _chat(UiPerson p, int lastMs, {int unread = 0}) => UiDirectChat(
  away: false,
  friend: p,
  online: true,
  last: 'hi',
  lastMine: false,
  lastMs: lastMs,
  unread: unread,
  timerMs: 0,
);

UiGroup _group(String id, String name, List<UiVoiceRoom> rooms) => UiGroup(
  id: id,
  name: name,
  localName: '',
  icon: 0,
  members: 4,
  online: 1,
  manager: false,
  rank: 1,
  outranks: const [],
  maxMembers: 20,
  maxInRoom: 8,
  voiceRooms: rooms,
  openInvites: 0,
);

AppState _state() {
  final state = AppState('/nonexistent/fogata-switcher-test')
    ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
    ..contacts.friends = [_friend(_leo), _friend(_ana), _friend(_bea)]
    ..contacts.groups = [
      _group('g1', 'Friday games', [
        const UiVoiceRoom(id: AppState.firstRoom, name: ''),
        const UiVoiceRoom(
          id: 'quiet',
          name: 'Quiet corner',
          barrier: UiBarrier(kind: UiBarrierKind.removed),
        ),
      ]),
      _group('g2', 'Book club', [
        const UiVoiceRoom(id: AppState.firstRoom, name: ''),
      ]),
    ];
  state.direct.chats = [_chat(_ana, 2000, unread: 2), _chat(_leo, 1000)];
  return state;
}

Future<void> _show(WidgetTester tester, AppState state, Size size) async {
  // The window's buttons ask the window manager, which tests do not have.
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
        builder: withToasts,
      ),
    ),
  );
  await tester.pump();
}

Future<void> _ctrlK(WidgetTester tester) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump(const Duration(milliseconds: 400));
}

List<String> _titles(List<Place> places) => [for (final p in places) p.title];

void main() {
  setUpAll(loadFonts);

  test(
    'with nothing typed: recent places, then chats newest first, rooms, groups',
    () {
      final state = _state();
      final places = allPlaces(state);
      expect(_titles(rankPlaces(places, '')), [
        'Ana', 'Leo', // chats, newest first
        'Book club', 'Friday games', // groups
        'Bea', // friends without a chat
        'General', 'General', 'Quiet corner', // voice rooms with nobody in them
      ]);
      final ranked = rankPlaces(
        places,
        '',
        recent: ['group:g1', 'chat:ana', 'chat:leo'],
        shown: 'chat:ana',
      );
      // The page shown is left out; the last visited come first.
      expect(_titles(ranked).take(3), ['Friday games', 'Leo', 'Book club']);
    },
  );

  test('typed: names that start with it first, then words, then within, then group', () {
    final places = allPlaces(_state());
    // Equals: a group before a friend with no chat yet; the group's room after both.
    expect(_titles(rankPlaces(places, 'b')), ['Book club', 'Bea', 'General']);
    expect(_titles(rankPlaces(places, 'club')), ['Book club', 'General']);
    expect(_titles(rankPlaces(places, 'orne')), ['Quiet corner']);
    // A group's name finds its voice rooms after the group itself.
    expect(_titles(rankPlaces(places, 'friday')), [
      'Friday games',
      'General',
      'Quiet corner',
    ]);
    expect(rankPlaces(places, 'zz'), isEmpty);
  });

  testWidgets('Ctrl K opens it on any page; typing and Enter go to a group', (
    tester,
  ) async {
    final state = _state();
    await _show(tester, state, const Size(960, 640));
    await _ctrlK(tester);
    expect(find.byKey(const ValueKey('switcher')), findsOneWidget);
    // Recent chats first, with their unread messages.
    expect(find.text('Ana'), findsWidgets);
    await tester.enterText(find.byKey(const ValueKey('switcher-field')), 'fri');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byKey(const ValueKey('switcher')), findsNothing);
    expect(state.nav.page, Screen.members);
    expect(state.nav.membersGroup, 'g1');
    // Esc closes it without going anywhere, and Ctrl K again too.
    await _ctrlK(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byKey(const ValueKey('switcher')), findsNothing);
    await _ctrlK(tester);
    await _ctrlK(tester);
    expect(find.byKey(const ValueKey('switcher')), findsNothing);
    expect(state.nav.page, Screen.members);
  });

  testWidgets(
    'it opens in the middle of the window, and typing keeps the field '
    'where it is',
    (tester) async {
      for (final size in const [
        Size(960, 640),
        Size(1280, 800),
        Size(1920, 1080),
        Size(1440, 1400),
      ]) {
        final state = _state();
        await _show(tester, state, size);
        await _ctrlK(tester);
        // Its spring settled.
        await tester.pump(const Duration(milliseconds: 400));
        final box = tester.getRect(find.byKey(const ValueKey('switcher')));
        expect(
          box.center.dx,
          moreOrLessEquals(size.width / 2),
          reason: '$size',
        );
        expect(
          box.center.dy,
          moreOrLessEquals(size.height / 2),
          reason: '$size',
        );
        final field = tester.getRect(
          find.byKey(const ValueKey('switcher-field')),
        );
        await tester.enterText(
          find.byKey(const ValueKey('switcher-field')),
          'book',
        );
        await tester.pump(const Duration(milliseconds: 400));
        expect(
          tester.getRect(find.byKey(const ValueKey('switcher'))).height,
          lessThan(box.height),
        );
        expect(
          tester.getRect(find.byKey(const ValueKey('switcher-field'))),
          field,
          reason: '$size',
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump(const Duration(milliseconds: 400));
      }
    },
  );

  testWidgets(
    'arrows choose a chat, and a voice room asks as the side column does',
    (tester) async {
      final state = _state();
      await _show(tester, state, const Size(1920, 1080));
      // In a wide window the title bar has the pill that opens it.
      await tester.tap(find.byKey(const ValueKey('find')));
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('switcher')), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump(const Duration(milliseconds: 400));
      expect(state.nav.page, Screen.direct);
      expect(state.direct.open, 'leo');
      // A room this device is kept out of says so, as the side column does.
      await _ctrlK(tester);
      await tester.enterText(
        find.byKey(const ValueKey('switcher-field')),
        'quiet',
      );
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Quiet corner is not for you now.'), findsOneWidget);
      expect(state.nav.page, Screen.direct);
      // The toast goes by itself.
      await tester.pump(const Duration(seconds: 10));
    },
  );

  testWidgets('arrows keep the chosen row whole inside the list', (
    tester,
  ) async {
    final state = _state()
      ..contacts.friends = [
        for (var i = 0; i < 20; i++)
          _friend(
            UiPerson(
              id: 'f$i',
              name: 'Friend ${i.toString().padLeft(2, '0')}',
              short: 'AB${i}C·D${i}EF',
              avatar: const UiAvatar(preset: 1),
            ),
          ),
      ];
    await _show(tester, state, const Size(1280, 800));
    await _ctrlK(tester);
    await tester.enterText(
      find.byKey(const ValueKey('switcher-field')),
      'friend',
    );
    await tester.pump();
    final list = tester.getRect(
      find.descendant(
        of: find.byKey(const ValueKey('switcher')),
        matching: find.byType(ListView),
      ),
    );
    void whole(int i) {
      final row = tester.getRect(
        find
            .ancestor(
              of: find.textContaining(
                'Friend ${i.toString().padLeft(2, '0')}',
                findRichText: true,
              ),
              matching: find.byType(IndexedSemantics),
            )
            .first,
      );
      expect(row.top, greaterThanOrEqualTo(list.top), reason: 'row $i');
      expect(row.bottom, lessThanOrEqualTo(list.bottom), reason: 'row $i');
    }

    for (var i = 1; i < 20; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      whole(i);
    }
    // Round to the first: the list is back at its very top.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    whole(0);
    final scroll = tester.widget<ListView>(
      find.descendant(
        of: find.byKey(const ValueKey('switcher')),
        matching: find.byType(ListView),
      ),
    );
    expect(scroll.controller!.offset, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    whole(19);
    expect(
      scroll.controller!.offset,
      scroll.controller!.position.maxScrollExtent,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 400));
  });

  testWidgets('Friends keeps its own search; Home lists every group', (
    tester,
  ) async {
    final state = _state()..nav.page = Screen.home;
    await _show(tester, state, const Size(1440, 900));
    expect(find.text('Friday games'), findsWidgets);
    expect(find.text('Book club'), findsWidgets);
    state.go(Screen.friends);
    await tester.pump();
    await tester.enterText(
      find.widgetWithText(TextField, 'Search friends'),
      'be',
    );
    await tester.pump();
    Finder inList(String name) =>
        find.descendant(of: find.byType(Friends), matching: find.text(name));
    expect(inList('Bea'), findsWidgets);
    expect(inList('Leo'), findsNothing);
    // The friend code the page makes needs the core: its notice goes by itself.
    await tester.pump(const Duration(seconds: 10));
  });
}
