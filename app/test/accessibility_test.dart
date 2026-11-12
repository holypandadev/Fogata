// Accessibility of the main screens: every control a screen reader reaches has a name,
// what floats over the window hides the page behind from it, rooms and chats say what
// the eye sees, the main flows work from the keyboard alone, long names and large text
// fit, and the kit's text reads at WCAG's contrast in every theme.

import 'dart:ui' show SemanticsAction, Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/announce.dart' show toldOf;
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/direct_state.dart' show newMessages;
import 'package:fogata/src/icons.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'cut_text.dart';
import 'fake_room.dart';
import 'keyboard_test.dart' show focused;
import 'text_contrast.dart';

/// The core's side of what the flows here ask of it: leaving the room and declining a
/// call are noted, and what else the pages read fails as with no core.
class FakeCore implements RustLibApi {
  final left = <String>[];
  final declined = <String>[];

  @override
  Future<void> crateApiAppLeaveRoom({required String groupId}) async =>
      left.add(groupId);

  @override
  Future<void> crateApiAppDeclineCall({required String groupId}) async =>
      declined.add(groupId);

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

final core = FakeCore();

const _sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

UiDirectItem _item(String id, String text, {bool mine = false}) => UiDirectItem(
  id: id,
  mine: mine,
  atMs: DateTime.now().millisecondsSinceEpoch,
  text: text,
  files: const [],
  delivered: true,
  expiresMs: 0,
  pinned: false,
);

UiDirectPage _page(List<UiDirectItem> items, {UiPerson friend = _sofia}) =>
    UiDirectPage(
      away: false,
      friend: friend,
      online: true,
      connected: true,
      items: items,
      more: false,
      pinned: const [],
      timerMs: 0,
      usage: const UiDirectUsage(messages: 1, pictures: 0, videos: 0, files: 0),
      altered: false,
    );

/// The chat with Sofía open, in a state made by [fakeRoom] with names [longer].
AppState _direct({String longer = ''}) {
  final friend = UiPerson(
    id: _sofia.id,
    name: '${_sofia.name}$longer',
    short: _sofia.short,
    avatar: _sofia.avatar,
  );
  final state = fakeRoom(people: 2, longer: longer)
    ..nav.page = Screen.direct
    ..roomState.roomGroup = null
    ..callState.call = null;
  state.direct
    ..open = friend.id
    ..page = _page([
      for (var i = 0; i < 4; i++)
        _item('$i', 'Message number $i', mine: i.isEven),
    ], friend: friend);
  return state;
}

AppState _home({String longer = ''}) {
  final state = fakeRoom(people: 3, longer: longer)..nav.page = Screen.home;
  state.direct.take([
    UiDirectChat(
      away: false,
      friend: UiPerson(
        id: _sofia.id,
        name: '${_sofia.name}$longer',
        short: _sofia.short,
        avatar: _sofia.avatar,
      ),
      online: true,
      unread: 3,
      last: 'Are we still on for Friday?',
      lastMine: false,
      lastMs: 0,
      timerMs: 0,
    ),
  ]);
  return state;
}

AppState _members({String longer = ''}) {
  final state = fakeRoom(people: 4, longer: longer)
    ..nav.page = Screen.members
    ..nav.membersGroup = 'g'
    ..roomState.roomGroup = null;
  state.contacts.members['g'] = [
    for (var i = 0; i < 4; i++)
      UiMember(
        away: i == 2,
        person: i == 0 ? state.contacts.profile!.person : other(i),
        rank: i,
        manager: i < 2,
        me: i == 0,
        waitingForKey: i == 3,
        fingerprint: '7Q4M K2XR 9PLD 3VHC',
      ),
  ];
  return state;
}

/// A call from Sofía, ringing here.
final _call = UiCall(
  groupId: 'call',
  with_: _sofia,
  outgoing: false,
  phase: UiCallPhase.incoming,
  sinceMs: DateTime.now().millisecondsSinceEpoch,
  members: const [_sofia],
  adding: const [],
  away: const [],
);

/// Makes [_call] ring in [state], or stop ringing.
void _ring(AppState state, {bool ringing = true}) {
  state.callState.calls = ringing ? [_call] : [];
  state.callState.ringAside = null;
  state.callState.changed();
}

/// The home page while this device is in no room, with [_call] ringing unless not
/// [ringing].
AppState _ringing({bool ringing = true}) {
  final state = fakeRoom(people: 2)
    ..nav.page = Screen.home
    ..roomState.roomGroup = null
    ..callState.call = null;
  state.contacts.groups = [
    for (final g in state.contacts.groups)
      UiGroup(
        id: g.id,
        name: g.name,
        localName: g.localName,
        icon: g.icon,
        members: g.members,
        online: g.online,
        manager: g.manager,
        rank: g.rank,
        outranks: g.outranks,
        maxMembers: g.maxMembers,
        maxInRoom: g.maxInRoom,
        voiceRooms: const [],
        openInvites: g.openInvites,
        needsUpdate: false,
      ),
  ];
  state.contacts.friends = [
    for (final person in [_sofia, other(0)])
      UiFriend(
        person: person,
        fingerprint: '${person.short} 9PLD 3VHC',
        presence: UiPresence.online,
        delivered: true,
      ),
  ];
  state.callState.calls = ringing ? [_call] : [];
  return state;
}

/// Shows [state] in the window, [size] and in the theme of [tokens], and lets it settle.
Future<void> _show(
  WidgetTester tester,
  AppState state, {
  Size size = const Size(1280, 800),
  Fg tokens = Fg.darkTokens,
}) async {
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
        theme: tokens.themeData,
        home: const Material(child: Shell()),
        builder: appFrame,
      ),
    ),
  );
  for (var i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
  // The pages also ask the core, which fails here.
  tester.takeException();
}

/// Takes the window away and lets what it left running end.
Future<void> _close(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

/// Presses Tab until the focus is on [label], at most [most] times; whether it got there.
Future<bool> _tabTo(WidgetTester tester, String label, {int most = 60}) async {
  for (var i = 0; i < most; i++) {
    if (focused() == label) return true;
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump(const Duration(milliseconds: 50));
  }
  return focused() == label;
}

/// Every label Tab reaches in [n] presses.
Future<Set<String?>> _tabs(WidgetTester tester, int n) async {
  final seen = <String?>{};
  for (var i = 0; i < n; i++) {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump(const Duration(milliseconds: 50));
    seen.add(focused());
  }
  return seen;
}

Future<void> _key(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyEvent(key);
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

void main() {
  setUpAll(() async {
    RustLib.initMock(api: core);
    await loadFonts();
  });
  tearDown(() => useLanguage('en'));

  group('every control a screen reader reaches has a name', () {
    final pages = <String, AppState Function()>{
      'room': () => fakeRoom(people: 4, cameras: 1),
      'home': _home,
      'direct chat': _direct,
      'friends': () => fakeRoom(people: 3)..nav.page = Screen.friends,
      'members': _members,
      'a call ringing': _ringing,
      'the bell': () =>
          _home()..toggleBell(const Rect.fromLTWH(1180, 4, 28, 28)),
      "the person's panel": () =>
          _home()..openUserPanel(const Rect.fromLTWH(8, 750, 290, 44)),
      'group settings': () => _members()..nav.groupSettings = 'g',
      for (final section in SettingsSection.values)
        'settings ${section.name}': () =>
            fakeRoom(people: 2)..openSettings(section),
    };
    for (final look in [Look.dark, Look.light]) {
      for (final MapEntry(key: name, value: make) in pages.entries) {
        testWidgets('$name, ${look.name}', (tester) async {
          final handle = tester.ensureSemantics();
          try {
            await _show(tester, make(), tokens: look.tokens);
            await expectLater(
              tester,
              meetsGuideline(labeledTapTargetGuideline),
            );
          } finally {
            handle.dispose();
          }
          await _close(tester);
        });
      }
    }
  });

  testWidgets('what floats over the window hides the page behind from screen '
      'readers', (tester) async {
    final handle = tester.ensureSemantics();
    final state = _ringing(ringing: false);
    await _show(tester, state);
    // Looked for in the tree screen readers get.
    final page = find.semantics.byLabel('Add a friend');
    expect(page, findsOneWidget);
    for (final (name, open) in <(String, void Function())>[
      ('settings', state.openSettings),
      ('group settings', () => state.openGroupSettings('g')),
      (
        "the person's panel",
        () => state.openUserPanel(const Rect.fromLTWH(8, 750, 290, 44)),
      ),
      (
        'the bell',
        () => state.toggleBell(const Rect.fromLTWH(1180, 4, 28, 28)),
      ),
      (
        "a friend's card",
        () => state.nav
          ..friendCard = _sofia.id
          ..friendCardFrom = const Rect.fromLTWH(320, 200, 400, 56)
          ..changed(),
      ),
      ('a call ringing', () => _ring(state)),
    ]) {
      open();
      await tester.pump(const Duration(milliseconds: 400));
      expect(page, findsNothing, reason: name);
      state
        ..closeSettings()
        ..closeGroupSettings()
        ..closeUserPanel()
        ..closeFriendCard();
      if (state.notices.bellFrom != null) state.toggleBell(Rect.zero);
      _ring(state, ringing: false);
      await tester.pump(const Duration(milliseconds: 400));
      expect(page, findsOneWidget, reason: name);
    }
    handle.dispose();
    await _close(tester);
  });

  testWidgets('a tile in a room says who it is and a microphone off, once', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    // Leo's camera is on and his microphone too; Sofía's microphone is off.
    await _show(tester, fakeRoom(people: 3, cameras: 1));
    // Leo's tile enlarges his camera: a button that says his name, once. His row in the
    // side column, which opens his card, is a button of its own that says so.
    final leo = find.semantics
        .byLabel(RegExp('Leo'))
        .evaluate()
        .map((node) => node.getSemanticsData())
        .where((data) => data.flagsCollection.isButton)
        .where((data) => !data.label.startsWith(l10n.friendsShowCard('Leo')))
        .toList();
    expect(leo.map((data) => data.label), ['Leo']);
    final sofia = find.semantics.byLabel(l10n.roomTileMicOff('Sofía'));
    expect(sofia, findsOne);
    // The volume menu is an action of the tile, not a tap that does nothing.
    final data = sofia.evaluate().single.getSemanticsData();
    expect(data.customSemanticsActionIds, isNotEmpty);
    expect(data.hasAction(SemanticsAction.tap), isFalse);
    handle.dispose();
    await _close(tester);
  });

  testWidgets(
    'a button with words says them once, and a switch says if it is on',
    (tester) async {
      final handle = tester.ensureSemantics();
      final state = _home();
      await _show(tester, state);
      expect(
        tester.getSemantics(find.bySemanticsLabel('Add a friend')).label,
        'Add a friend',
      );
      state.openSettings(SettingsSection.behaviour);
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      final switches = find.byType(FgSwitch);
      expect(switches, findsWidgets);
      for (final element in switches.evaluate()) {
        final data = tester
            .getSemantics(find.byWidget(element.widget))
            .getSemanticsData();
        expect(data.flagsCollection.isToggled, isNot(Tristate.none));
        expect(data.label, isNot(contains('\n')));
      }
      handle.dispose();
      await _close(tester);
    },
  );

  testWidgets('a call ringing is read out, and the keyboard answers it', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final state = _ringing();
    await _show(tester, state);
    final calling = tester
        .getSemantics(find.bySemanticsLabel(l10n.callLayersIsCalling('Sofía')))
        .getSemanticsData();
    expect(calling.flagsCollection.isLiveRegion, isTrue);
    // Tab goes round the card and not the page behind it.
    final seen = await _tabs(tester, 12);
    expect(seen, containsAll([l10n.commonDecline, l10n.commonAccept]));
    expect(seen, isNot(contains('Add a friend')));
    // Esc puts it aside, as a click beside it does.
    await _key(tester, LogicalKeyboardKey.escape);
    expect(state.callState.ringAside, 'call');
    handle.dispose();
    await _close(tester);
  });

  testWidgets('Decline is reached and pressed with the keyboard', (
    tester,
  ) async {
    core.declined.clear();
    await _show(tester, _ringing());
    expect(await _tabTo(tester, l10n.commonDecline), isTrue);
    await _key(tester, LogicalKeyboardKey.enter);
    expect(core.declined, ['call']);
    await _close(tester);
  });

  test('a message coming into the chat on screen is told, once', () {
    final before = _page([_item('1', 'Hello'), _item('2', 'Hi', mine: true)]);
    final after = _page([
      ...before.items,
      _item('3', 'Are we still on for Friday?'),
      _item('4', 'Yes', mine: true),
    ]);
    expect(newMessages(before, after), [
      ('Sofía', 'Are we still on for Friday?'),
    ]);
    // Opening a chat is not a message coming, nor is going to another.
    expect(newMessages(null, after), isEmpty);
    expect(newMessages(after, after), isEmpty);
    expect(newMessages(before, _page(after.items, friend: other(0))), isEmpty);
    // Older messages read for scrolling up, or for one to show, are not new.
    final older = _page([
      for (var i = 0; i < 200; i++) _item('old$i', 'Old $i'),
      ...after.items,
    ]);
    expect(newMessages(after, older), isEmpty);
    // Nor when one came meanwhile: only that one is.
    expect(
      newMessages(after, _page([...older.items, _item('5', 'And you?')])),
      [('Sofía', 'And you?')],
    );
  });

  test('messages are told one by one while few, else how many', () {
    final two = [('Sofía', 'Hello'), ('Leo', 'Hi')];
    expect(toldOf(two), [
      l10n.commonNewMessageFrom('Sofía', 'Hello'),
      l10n.commonNewMessageFrom('Leo', 'Hi'),
    ]);
    final five = [for (var i = 0; i < 5; i++) ('Sofía', 'Message $i')];
    expect(toldOf(five), [l10n.commonNewMessagesFrom(5, 'Sofía')]);
    expect(toldOf([...five, ('Leo', 'Hi')]), [l10n.commonNewMessages(6)]);
  });

  test("a room's chat as it was when first read is not told; what comes after "
      'is', () {
    final state = fakeRoom(people: 3);
    final room = state.roomState.room!;
    UiRoom withChat(List<UiChatLine> chat, {List<UiFile> files = const []}) =>
        UiRoom(
          startedMs: room.startedMs,
          joined: true,
          participants: room.participants,
          chat: chat,
          files: files,
        );
    UiChatLine line(String id, {bool mine = false}) => UiChatLine(
      id: id,
      sender: mine ? 'me' : 'p0',
      nick: mine ? 'Marta' : 'Leo',
      text: 'Line $id',
      timestampMs: 0,
      mine: mine,
    );
    String nameOf(UiChatLine line) => line.nick;
    // The memory the groups make, before the room is read: the history is not new.
    state.roomState.memories.clear();
    state.roomState.rememberRooms(state.contacts.groups);
    final memory = state.roomState.memories['g'];
    expect(memory, isNotNull);
    final history = withChat([for (var i = 0; i < 30; i++) line('$i')]);
    expect(arrivals(memory, history, nameOf), isEmpty);
    state.roomState.remember('g', history);
    // Once read, a line and a file of the others are new; this device's own are not.
    final file = UiFile(
      sharedMs: 0,
      id: 'f',
      name: 'photo.jpg',
      size: 1000,
      sharedBy: other(1),
      holders: [other(1)],
      mine: false,
      sent: 0,
    );
    final later = withChat(
      [...history.chat, line('new'), line('mine', mine: true)],
      files: [file],
    );
    expect(arrivals(memory, later, nameOf), [
      ('Leo', 'Line new'),
      ('Sofía', 'photo.jpg'),
    ]);
    state.dispose();
  });

  group('the keyboard alone', () {
    testWidgets('opens the settings, stays in them and comes back with Esc', (
      tester,
    ) async {
      final state = _home();
      await _show(tester, state);
      expect(await _tabTo(tester, 'Settings'), isTrue);
      await _key(tester, LogicalKeyboardKey.enter);
      expect(state.nav.settingsOpen, isTrue);
      final seen = await _tabs(tester, 30);
      expect(seen, contains(l10n.settingsCloseTooltip));
      expect(seen, isNot(contains('Add a friend')));
      await _key(tester, LogicalKeyboardKey.escape);
      expect(state.nav.settingsOpen, isFalse);
      // The focus is back where it was before.
      expect(focused(), 'Settings');
      await _close(tester);
    });

    testWidgets(
      'a call that rings while writing leaves the focus in the box; F6 '
      'goes to it',
      (tester) async {
        final state = _direct();
        state.contacts.groups = _ringing(ringing: false).contacts.groups;
        await _show(tester, state);
        final field = l10n.directMessagePlaceholder('Sofía');
        expect(await _tabTo(tester, field), isTrue);
        _ring(state);
        for (var i = 0; i < 4; i++) {
          await tester.pump(const Duration(milliseconds: 200));
        }
        expect(find.text(l10n.callLayersIsCalling('Sofía')), findsOneWidget);
        // Still writing: the words go to the box, and Esc is the box's.
        expect(focused(), field);
        tester.testTextInput.enterText('Hello');
        await tester.pump();
        expect(find.text('Hello'), findsOneWidget);
        await _key(tester, LogicalKeyboardKey.escape);
        expect(state.callState.ringAside, isNull);
        // F6 goes to the card; Esc there puts the call aside, back in the box.
        await _key(tester, LogicalKeyboardKey.f6);
        final seen = await _tabs(tester, 8);
        expect(seen, contains(l10n.commonDecline));
        expect(seen, isNot(contains(field)));
        await _key(tester, LogicalKeyboardKey.escape);
        expect(state.callState.ringAside, 'call');
        await tester.pump(const Duration(milliseconds: 400));
        expect(focused(), field);
        await _close(tester);
      },
    );

    testWidgets('a layer that closes over another gives the focus back to it', (
      tester,
    ) async {
      final state = _ringing(ringing: false);
      await _show(tester, state);
      expect(await _tabTo(tester, 'Settings'), isTrue);
      await _key(tester, LogicalKeyboardKey.enter);
      expect(state.nav.settingsOpen, isTrue);
      await _tabs(tester, 2);
      final inSettings = focused();
      // A call rings over the settings and is put aside.
      _ring(state);
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      await _key(tester, LogicalKeyboardKey.escape);
      expect(state.callState.ringAside, 'call');
      await tester.pump(const Duration(milliseconds: 400));
      // Back where it was in the settings: Tab stays in them, and Esc closes them.
      expect(focused(), inSettings);
      expect(await _tabs(tester, 30), isNot(contains('Add a friend')));
      await _key(tester, LogicalKeyboardKey.escape);
      expect(state.nav.settingsOpen, isFalse);
      expect(focused(), 'Settings');
      await _close(tester);
    });

    testWidgets(
      'settings opened from the panel go back to what opened the panel',
      (tester) async {
        final state = _ringing(ringing: false);
        await _show(tester, state);
        expect(await _tabTo(tester, 'Your profile'), isTrue);
        await _key(tester, LogicalKeyboardKey.enter);
        expect(state.nav.userPanelFrom, isNotNull);
        await _tabs(tester, 1);
        // The panel's way to the settings: the panel goes as they come.
        state
          ..closeUserPanel()
          ..openSettings();
        for (var i = 0; i < 4; i++) {
          await tester.pump(const Duration(milliseconds: 200));
        }
        await _key(tester, LogicalKeyboardKey.escape);
        expect(state.nav.settingsOpen, isFalse);
        expect(focused(), 'Your profile');
        await _close(tester);
      },
    );

    testWidgets('writes and sends a message', (tester) async {
      final state = _direct();
      final sent = <String>[];
      state.direct.sendDirect = ({
        required deviceId,
        required text,
        required files,
      }) async => sent.add('$deviceId: $text');
      await _show(tester, state);
      final field = l10n.directMessagePlaceholder('Sofía');
      expect(await _tabTo(tester, field), isTrue);
      tester.testTextInput.enterText('Hi, Sofía');
      await tester.pump();
      await tester.testTextInput.receiveAction(TextInputAction.done);
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 100));
      }
      expect(sent, ['sofia: Hi, Sofía']);
      // The focus stays in the box, for the next one.
      expect(focused(), field);
      await _close(tester);
    });

    testWidgets('goes back into the call, mutes and leaves it', (tester) async {
      core.left.clear();
      final state = fakeRoom(people: 3)..nav.page = Screen.home;
      await _show(tester, state);
      expect(await _tabTo(tester, 'Return to room'), isTrue);
      await _key(tester, LogicalKeyboardKey.enter);
      expect(state.nav.page, Screen.room);
      expect(await _tabTo(tester, 'Mute microphone'), isTrue);
      await _key(tester, LogicalKeyboardKey.space);
      expect(state.callState.call!.mic, isFalse);
      // Leave, or Cancel while the call still connects.
      final leave = tester.widget<FgButton>(
        find.byKey(const ValueKey('leave')),
      );
      expect(await _tabTo(tester, leave.text ?? leave.tooltip!), isTrue);
      await _key(tester, LogicalKeyboardKey.enter);
      await tester.pump(const Duration(seconds: 1));
      expect(core.left, ['g']);
      expect(state.nav.page, isNot(Screen.room));
      await _close(tester);
    });
  });

  group('long names fit at twice the text size', () {
    // Names as long as a person or a group may well have, in German.
    const longer = ' Maximiliane von Hohenzollern-Sigmaringen';
    final pages = <String, AppState Function()>{
      'room': () => fakeRoom(people: 4, cameras: 1, longer: longer),
      'home': () => _home(longer: longer),
      'direct chat': () => _direct(longer: longer),
      'members': () => _members(longer: longer),
      'friends': () =>
          fakeRoom(people: 3, longer: longer)..nav.page = Screen.friends,
      "the person's panel": () =>
          _home(longer: longer)
            ..openUserPanel(const Rect.fromLTWH(8, 590, 290, 44)),
      'group settings': () => _members(longer: longer)..nav.groupSettings = 'g',
    };
    for (final MapEntry(key: name, value: make) in pages.entries) {
      testWidgets(name, (tester) async {
        tester.platformDispatcher.textScaleFactorTestValue = 2;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        final found = <String>[];
        final before = FlutterError.onError;
        FlutterError.onError = (details) {
          final text = details.toString();
          if (text.contains('overflowed')) found.add(text.split('\n').first);
        };
        try {
          await _show(
            tester,
            make()..prefs.language = 'de',
            size: const Size(960, 640),
          );
          found.addAll(cutText());
        } finally {
          FlutterError.onError = before;
        }
        expect(found, isEmpty);
        await _close(tester);
      });
    }
  });

  group("the kit's text reads at WCAG's contrast", () {
    for (final look in Look.values.where((l) => l != Look.system)) {
      testWidgets(look.name, (tester) async {
        final handle = tester.ensureSemantics();
        final c = look.tokens;
        tester.view.physicalSize = const Size(2400, 1600);
        tester.view.devicePixelRatio = 2;
        addTearDown(tester.view.reset);
        Widget on(Color surface, List<Widget> children) => ColoredBox(
          color: surface,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Wrap(spacing: 12, runSpacing: 12, children: children),
          ),
        );
        List<Widget> words() => [
          Text('Words', style: tx(c.fg, size: 14)),
          Text('Words', style: tx(c.fg2, size: 14)),
          Text('Words', style: tx(c.fg3, size: 14)),
          Text('Words', style: tx(c.red, size: 14)),
          Text('Words', style: tx(c.green, size: 14)),
          Text('Words', style: tx(c.amber, size: 14)),
          Text('Words', style: tx(c.blue, size: 14)),
        ];
        await tester.pumpWidget(
          MaterialApp(
            theme: look.tokens.themeData,
            home: Material(
              color: c.bg,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  on(c.bg, [
                    ...words(),
                    for (final kind in Kind.values)
                      FgButton(text: 'Button', kind: kind, onTap: () {}),
                    Pill(text: 'Live', color: c.green, background: c.greenSoft),
                    Pill(text: 'Away', color: c.amber, background: c.amberSoft),
                    Pill(text: 'Beta', color: c.red, background: c.redSoft),
                    Pill(text: 'New', color: c.blue, background: c.blueSoft),
                    const CountBadge(4),
                  ]),
                  on(c.bg1, words()),
                  on(c.bg2, words()),
                  on(c.raised, words()),
                  on(c.bg, [
                    SizedBox(
                      width: 320,
                      child: Segmented(
                        tabs: const [('Online', null, null), ('All', 3, null)],
                        selected: 0,
                        onSelect: (_) {},
                      ),
                    ),
                    SizedBox(
                      width: 220,
                      child: NavItem(
                        text: 'Profile',
                        icon: Ic.user,
                        selected: true,
                        onTap: () {},
                      ),
                    ),
                    SizedBox(
                      width: 220,
                      child: NavItem(
                        text: 'Appearance',
                        icon: Ic.settings,
                        onTap: () {},
                      ),
                    ),
                  ]),
                ],
              ),
            ),
          ),
        );
        await tester.pump(const Duration(seconds: 1));
        try {
          await expectLater(tester, meetsGuideline(tokenTextContrastGuideline));
        } finally {
          handle.dispose();
        }
      });
    }

    // The check itself fails text that does not read: grey on grey, a colour that
    // reads on its own surface but not through a translucent fill or a faded layer
    // over it, and the chosen tab of a Segmented, whose thumb is beside its text and
    // not around it, when the thumb is too close to the text.
    testWidgets('and fails text that does not', (tester) async {
      Future<bool> reads(Widget child) async {
        await tester.pumpWidget(
          Directionality(
            textDirection: TextDirection.ltr,
            child: ColoredBox(
              color: const Color(0xFF000000),
              child: Center(child: child),
            ),
          ),
        );
        final result = await tokenTextContrastGuideline.evaluate(tester);
        return result.passed;
      }

      const white = TextStyle(color: Color(0xFFFFFFFF), fontSize: 14);
      expect(await reads(const Text('Words', style: white)), isTrue);
      expect(
        await reads(
          const Text(
            'Words',
            style: TextStyle(color: Color(0xFF555555), fontSize: 14),
          ),
        ),
        isFalse,
      );
      // 3.6:1 is enough for large text, and not for small.
      const grey = Color(0xFF6E6E6E);
      expect(
        await reads(
          const Text('Words', style: TextStyle(color: grey, fontSize: 18)),
        ),
        isTrue,
      );
      expect(
        await reads(
          const Text('Words', style: TextStyle(color: grey, fontSize: 14)),
        ),
        isFalse,
      );
      expect(
        await reads(
          const ColoredBox(
            color: Color(0xCCFFFFFF),
            child: Text('Words', style: white),
          ),
        ),
        isFalse,
      );
      expect(
        await reads(
          const Opacity(opacity: .3, child: Text('Words', style: white)),
        ),
        isFalse,
      );
      expect(
        await reads(
          const Stack(
            children: [
              Positioned.fill(child: ColoredBox(color: Color(0xFFEEEEEE))),
              Text('Words', style: white),
            ],
          ),
        ),
        isFalse,
      );
    });
  });
}
