import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/direct_state.dart';
import 'package:fogata/src/format.dart' show bytes;
import 'package:fogata/src/moving.dart' show playingAnimations;
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/friend_card.dart';
import 'package:fogata/src/screens/user_panel.dart' show shellLayerKey;
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'wait.dart';

const me = UiPerson(
  id: 'me',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

const sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

const usage = UiDirectUsage(messages: 400000, pictures: 0, videos: 0, files: 0);

final now = DateTime.now().millisecondsSinceEpoch;
final yesterday = DateTime.now()
    .subtract(const Duration(days: 1))
    .millisecondsSinceEpoch;

UiDirectItem message(
  String id,
  String text, {
  bool mine = false,
  int? at,
  bool delivered = true,
  bool pinned = false,
  List<UiDirectFile> files = const [],
}) => UiDirectItem(
  id: id,
  mine: mine,
  atMs: at ?? now,
  text: text,
  files: files,
  delivered: delivered,
  expiresMs: 0,
  pinned: pinned,
);

UiDirectPage page(
  List<UiDirectItem> items, {
  bool online = false,
  int? ended,
  int timer = 0,
  bool more = false,
  bool altered = false,
}) => UiDirectPage(
  away: false,
  friend: sofia,
  online: online,
  lastSeenMs: online ? null : now - 2 * dayMs,
  connected: online,
  items: items,
  more: more,
  pinned: [
    for (final i in items.reversed)
      if (i.pinned) i,
  ],
  timerMs: timer,
  endedMs: ended,
  usage: usage,
  altered: altered,
);

Future<AppState> show(WidgetTester tester, UiDirectPage shown) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState('/nonexistent/fogata-direct-test')
    ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
    ..nav.page = Screen.direct;
  state.direct
    ..open = sofia.id
    ..page = shown;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: DirectPage()),
      ),
    ),
  );
  await tester.pump();
  // The test font draws every letter as a wide square: long lines may overflow here.
  tester.takeException();
  return state;
}

/// A GIF [side] pixels square with [frames] frames of one pixel each, shown one after the
/// other with no wait: tens of bytes that ask for a full-size frame for each.
Uint8List animation(int side, int frames) {
  final size = ByteData(4)
    ..setUint16(0, side, Endian.little)
    ..setUint16(2, side, Endian.little);
  return Uint8List.fromList([
    ...'GIF89a'.codeUnits,
    ...size.buffer.asUint8List(),
    0x80, 0, 0, // a global table of two colours
    0, 0, 0, 0xff, 0xff, 0xff,
    for (var i = 0; i < frames; i++) ...[
      0x21, 0xf9, 4, 0, 0, 0, 0, 0, // no wait before the next frame
      0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, // one pixel at the top left
      2, 2, 0x44, 0x01, 0, // clear, colour 0, end
    ],
    0x3b,
  ]);
}

/// Whether the item of [key] shows a picture.
bool pictureUnder(Key key) => find
    .descendant(of: find.byKey(key), matching: find.byType(Image))
    .evaluate()
    .isNotEmpty;

/// How many frames the first picture under [item] shows: its first, waited for however
/// long decoding takes, then any more while the chat stays open about half a second.
Future<int> framesShown(WidgetTester tester, Finder item) async {
  final image = find.descendant(of: item, matching: find.byType(Image)).first;
  var shown = 0;
  final stream = tester
      .widget<Image>(image)
      .image
      .resolve(createLocalImageConfiguration(tester.element(image)));
  final listener = ImageStreamListener((info, _) {
    shown++;
    info.dispose();
  });
  stream.addListener(listener);
  await pumpUntil(tester, () => shown > 0, what: 'the picture\'s first frame');
  // More frames would come within this, real time and the test's both passing.
  for (var i = 0; i < 25; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  stream.removeListener(listener);
  return shown;
}

void main() {
  test('days, sizes, timers and limits in words', () {
    final today = DateTime(2026, 10, 25, 12);
    int at(int days) =>
        today.subtract(Duration(days: days)).millisecondsSinceEpoch;
    expect(dayLabel(at(0), now: today), 'Today');
    expect(dayLabel(at(1), now: today), 'Yesterday');
    expect(dayLabel(at(3), now: today), 'Thursday');
    expect(dayLabel(at(30), now: today), 'Sep 25');
    expect(dayLabel(at(400), now: today), 'Sep 20 2025');
    expect(storageSize(0), '0 MB');
    expect(storageSize(400000), '0.4 MB');
    expect(storageSize(84000000), '84 MB');
    expect(storageSize(1200000000), '1.2 GB');
    expect(
      [for (final t in timers) timerWord(t)],
      ['Off', '24 hours', '7 days', '90 days'],
    );
    expect(
      [for (final l in storageLimits) limitWord(l)],
      ['500 MB', '1 GB', '2 GB', '5 GB', '10 GB', 'No limit'],
    );
  });

  test('a change of the timer says who made it', () {
    UiDirectItem timer(int ms, {bool mine = false}) => UiDirectItem(
      id: 't',
      mine: mine,
      atMs: 1,
      text: '',
      files: const [],
      timerMs: ms,
      delivered: true,
      expiresMs: 0,
      pinned: false,
    );
    expect(
      timerLine(timer(7 * dayMs, mine: true), 'Sofía', first: false),
      'You turned on disappearing messages: 7 days',
    );
    expect(
      timerLine(timer(0), 'Sofía', first: false),
      'Sofía turned off disappearing messages',
    );
    expect(
      timerLine(timer(dayMs), 'Sofía', first: true),
      'Disappearing messages on: 24 hours',
    );
  });

  testWidgets('messages go by day and by person, with what waits', (
    tester,
  ) async {
    await show(
      tester,
      page([
        message('1', 'How did the interview go?', at: yesterday),
        message('2', 'Tell me', at: yesterday + 60000),
        message(
          '3',
          'Good! I will tell you tomorrow',
          mine: true,
          delivered: false,
        ),
      ]),
    );
    expect(find.text('Yesterday'), findsOneWidget);
    expect(find.text('Today'), findsOneWidget);
    // Two messages a minute apart go under one name.
    expect(find.textContaining('Sofía', findRichText: true), findsWidgets);
    expect(find.text('Tell me'), findsOneWidget);
    expect(
      find.text('Waiting for Sofía to come online · sent when you both are'),
      findsOneWidget,
    );
    expect(find.text('Offline · seen 2 days ago'), findsOneWidget);
    expect(
      find.text('Sofía is offline. You can write when you’re both online.'),
      findsOneWidget,
    );
    expect(find.text('THIS CHAT ON THIS DEVICE'), findsOneWidget);
    expect(find.text('0.4 MB'), findsOneWidget);
    // Offline, the call button is there but does nothing.
    final call = tester.widget<FgButton>(
      find.byKey(const ValueKey('direct-call')),
    );
    expect(call.onTap, isNull);
  });

  testWidgets('nothing can be written unless both are online', (tester) async {
    // Sofía is offline: the chat reads, but the box is closed and says why.
    final state = await show(tester, page([message('1', 'Hello')]));
    expect(find.byKey(const ValueKey('direct-locked')), findsOneWidget);
    expect(find.byKey(const ValueKey('direct-message')), findsNothing);
    expect(
      find.text('Sofía is offline. You can write when you’re both online.'),
      findsOneWidget,
    );
    expect(find.text('Hello'), findsWidgets);
    // Nor the timer, nor pins: they too have to reach her.
    expect(
      tester.widget<Springy>(find.byKey(const ValueKey('direct-timer'))).onTap,
      isNull,
    );
    expect(find.byKey(const ValueKey('direct-pin:1')), findsNothing);

    // Both online: the box is back.
    state.direct.page = page([message('1', 'Hello')], online: true);
    state.notifyListeners();
    await tester.pump();
    expect(find.byKey(const ValueKey('direct-locked')), findsNothing);
    expect(find.byKey(const ValueKey('direct-message')), findsOneWidget);
    expect(
      tester.widget<Springy>(find.byKey(const ValueKey('direct-timer'))).onTap,
      isNotNull,
    );

    // Appearing offline closes it too, with its own words.
    state.prefs.appearOffline = true;
    state.notifyListeners();
    await tester.pump();
    expect(
      find.text('You appear offline. Go online to write.'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('direct-message')), findsNothing);
    expect(writeBlocked(state, state.direct.page!), isNotNull);
    state.prefs.appearOffline = false;
    expect(writeBlocked(state, state.direct.page!), isNull);
  });

  testWidgets('messages either side of midnight go under two days', (
    tester,
  ) async {
    final today = DateTime.now();
    final midnight = DateTime(
      today.year,
      today.month,
      today.day,
    ).millisecondsSinceEpoch;
    await show(
      tester,
      page([
        message('1', 'Before', at: midnight - 30000),
        message('2', 'Just before', at: midnight - 1),
        message('3', 'Right at midnight', at: midnight),
        message('4', 'After', at: midnight + 1000),
      ]),
    );
    expect(find.text('Yesterday'), findsOneWidget);
    expect(find.text('Today'), findsOneWidget);
    expect(find.text('Just before'), findsOneWidget);
    expect(find.text('Right at midnight'), findsOneWidget);
  });

  testWidgets('a message that arrived says so, and online the call works', (
    tester,
  ) async {
    await show(tester, page([message('1', 'Hello', mine: true)], online: true));
    expect(find.text('Delivered'), findsOneWidget);
    expect(find.text('Online'), findsOneWidget);
    expect(
      find.text(
        'Pictures up to 10 MB download automatically; videos and files when you open them.',
      ),
      findsOneWidget,
    );
    final call = tester.widget<FgButton>(
      find.byKey(const ValueKey('direct-call')),
    );
    expect(call.onTap, isNotNull);
  });

  testWidgets('a removed attachment can be asked for again', (tester) async {
    const gone = UiDirectFile(
      id: 'f',
      name: 'budget.pdf',
      size: 220000,
      kind: UiFileKind.file,
      state: UiFileState.removed,
      received: 0,
    );
    await show(
      tester,
      page([
        message('1', '', files: [gone]),
      ], online: true),
    );
    expect(find.text('Removed here to free space'), findsOneWidget);
    expect(find.text('Ask again'), findsOneWidget);
  });

  testWidgets('a file not here yet says when it can come', (tester) async {
    const there = UiDirectFile(
      id: 'f',
      name: 'budget.pdf',
      size: 220000,
      kind: UiFileKind.file,
      state: UiFileState.missing,
      received: 0,
    );
    final items = [
      message('1', '', files: [there]),
    ];
    final get = find.byKey(const ValueKey('direct-get:f'));
    // Sofía is offline: nothing to press, and it says when.
    final state = await show(tester, page(items));
    expect(
      find.text('220 KB · available when Sofía is online'),
      findsOneWidget,
    );
    expect(tester.widget<FgButton>(get).onTap, isNull);
    // Both online: it can be fetched.
    state.direct.page = page(items, online: true);
    state.notifyListeners();
    await tester.pump();
    expect(find.text('220 KB'), findsOneWidget);
    expect(tester.widget<FgButton>(get).onTap, isNotNull);
    // Appearing offline, this device receives nothing either.
    state.prefs.appearOffline = true;
    state.notifyListeners();
    await tester.pump();
    expect(find.text('220 KB · available when you go online'), findsOneWidget);
    expect(tester.widget<FgButton>(get).onTap, isNull);
  });

  testWidgets('a picture too large to keep in memory shows as a file', (
    tester,
  ) async {
    UiDirectFile picture(String id, int size) => UiDirectFile(
      id: id,
      name: 'lake-$id.jpg',
      size: size,
      kind: UiFileKind.picture,
      state: UiFileState.here,
      received: size,
      path: '/nonexistent/fogata-direct-test/$id.jpg',
    );
    await show(
      tester,
      page([
        message('1', '', files: [picture('a', 3 << 30)]),
        message('2', '', files: [picture('b', 200000)]),
      ], online: true),
    );
    // The large one is never read: a file card at once, to save.
    expect(find.text('lake-a.jpg'), findsOneWidget);
    expect(find.text('Save'), findsOneWidget);
    expect(find.textContaining('lake-b.jpg ·'), findsOneWidget);
  });

  test('how long until a message disappears, in words', () {
    const minute = 60000;
    const hour = 60 * minute;
    expect(disappearsIn(5 * minute, nowMs: 0), 'Disappears in 5 min');
    expect(disappearsIn(20000, nowMs: 0), 'Disappears in 1 min');
    expect(disappearsIn(-1000, nowMs: 0), 'Disappears in 1 min');
    expect(disappearsIn(59 * minute + 50000, nowMs: 0), 'Disappears in 1 h');
    expect(disappearsIn(3 * hour - 10 * minute, nowMs: 0), 'Disappears in 3 h');
    expect(disappearsIn(dayMs - minute, nowMs: 0), 'Disappears in 24 h');
    expect(disappearsIn(2 * dayMs, nowMs: 0), 'Disappears in 2 days');
    expect(disappearsIn(7 * dayMs - hour, nowMs: 0), 'Disappears in 7 days');
  });

  testWidgets('hovering a message with a timer tells when it goes from here', (
    tester,
  ) async {
    UiDirectItem lasting(String id, String text, int expires) => UiDirectItem(
      id: id,
      mine: false,
      atMs: now,
      text: text,
      files: const [],
      delivered: true,
      expiresMs: expires,
      pinned: false,
    );
    await show(
      tester,
      page([
        message('1', 'Written before the timer'),
        lasting('2', 'See you later', now + 3 * 3600 * 1000 - 60000),
      ], timer: dayMs),
    );
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    // No countdown on the messages themselves.
    expect(find.textContaining('Disappears'), findsNothing);
    await mouse.moveTo(tester.getCenter(find.text('See you later')));
    await tester.pump(const Duration(milliseconds: 600));
    expect(find.text('Disappears in 3 h'), findsOneWidget);
    // A message without a timer has nothing to tell.
    await mouse.moveTo(tester.getCenter(find.text('Written before the timer')));
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 600));
    expect(find.textContaining('Disappears'), findsNothing);
  });

  testWidgets('the timer pill opens the four choices', (tester) async {
    await show(
      tester,
      page([message('1', 'Hello')], online: true, timer: 7 * dayMs),
    );
    expect(find.text('7 days'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('direct-timer')));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(find.text('Disappearing messages'), findsWidgets);
    for (final t in timers) {
      expect(find.byKey(ValueKey('timer:$t')), findsOneWidget);
    }
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
  });

  testWidgets('a chat whose history changed on the disk says so', (
    tester,
  ) async {
    await show(tester, page([message('1', 'Hello')], online: true));
    expect(find.byKey(const ValueKey('direct-altered')), findsNothing);
    await show(
      tester,
      page([message('1', 'Hello')], online: true, altered: true),
    );
    expect(find.byKey(const ValueKey('direct-altered')), findsOneWidget);
    expect(find.textContaining('Some messages may be missing'), findsOneWidget);
    expect(find.text('Hello'), findsOneWidget);
  });

  testWidgets('no longer friends: the chat can only be read or deleted', (
    tester,
  ) async {
    await show(
      tester,
      page([
        message('1', 'Bye'),
        message('2', 'Shall we go on?', mine: true, delivered: false),
      ], ended: now),
    );
    expect(find.text('Not delivered'), findsOneWidget);
    expect(find.text('Delete chat'), findsOneWidget);
    expect(find.byKey(const ValueKey('direct-message')), findsNothing);
    expect(find.byKey(const ValueKey('direct-timer')), findsNothing);
    expect(find.text('No longer a friend'), findsOneWidget);
  });

  UiDirectItem pinOf(String target, {bool mine = false, bool pinned = true}) =>
      UiDirectItem(
        id: 'pin-$target-$pinned',
        mine: mine,
        atMs: now,
        text: '',
        files: const [],
        delivered: true,
        expiresMs: 0,
        pinned: false,
        pin: UiDirectPin(target: target, pinned: pinned),
      );

  test('a pin says who pinned', () {
    expect(pinLine(pinOf('1', mine: true), 'Sofía'), 'You pinned a message');
    expect(pinLine(pinOf('1'), 'Sofía'), 'Sofía pinned a message');
  });

  testWidgets(
    'a pinned message says so; its pin is a line, its unpin nothing',
    (tester) async {
      await show(
        tester,
        page([
          message('1', 'The address is 4 High Street', pinned: true),
          pinOf('1'),
          message('2', 'Thanks', mine: true),
          pinOf('2', mine: true, pinned: false),
        ]),
      );
      expect(find.text('Pinned'), findsOneWidget);
      expect(find.text('Sofía pinned a message'), findsOneWidget);
      expect(find.textContaining('unpinned'), findsNothing);
      // The pill says how many are pinned.
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('direct-pins')),
          matching: find.text('1'),
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'hovering a message offers the pin, until the chat has the most',
    (tester) async {
      await show(
        tester,
        page([
          for (var n = 0; n < maxPins; n++)
            message('p$n', 'Pinned $n', at: yesterday + n, pinned: true),
          message('free', 'Not pinned'),
        ], online: true),
      );
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(tester.getCenter(find.text('Not pinned')));
      await tester.pumpAndSettle();
      final pin = find.byKey(const ValueKey('direct-pin:free'));
      expect(tester.widget<Springy>(pin).onTap, isNull);
      expect(tester.widget<Springy>(pin).tooltip, contains('unpin one first'));
      // A pinned one can always be unpinned.
      expect(
        tester
            .widget<Springy>(find.byKey(const ValueKey('direct-pin:p49')))
            .onTap,
        isNotNull,
      );
    },
  );

  testWidgets('a pin line brings its message into view, far up the chat', (
    tester,
  ) async {
    final state = await show(
      tester,
      page([
        message('0', 'The very first one', at: yesterday, pinned: true),
        for (var n = 1; n < 80; n++)
          message('$n', 'Message $n', at: yesterday + n * 600000),
        pinOf('0', mine: true),
      ]),
    );
    expect(find.text('The very first one'), findsNothing);
    await tester.tap(find.text('You pinned a message'));
    await tester.pumpAndSettle();
    tester.takeException();
    final shown = find.text('The very first one');
    expect(shown, findsOneWidget);
    expect(tester.getCenter(shown).dy, inInclusiveRange(56, 900));
    expect(state.direct.jump.value?.$1, '0');
  });

  testWidgets('the pins pill lists them, the latest first, and opens one', (
    tester,
  ) async {
    final state = await show(
      tester,
      page([
        message('a', 'First', at: yesterday, pinned: true),
        message('b', 'Second', mine: true, pinned: true),
      ], online: true),
    );
    await tester.tap(find.byKey(const ValueKey('direct-pins')));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(find.text('Pinned messages'), findsOneWidget);
    final rows = find.byWidgetPredicate(
      (w) =>
          w.key is ValueKey &&
          '${(w.key! as ValueKey).value}'.startsWith('pinned:'),
    );
    expect(
      [for (final e in rows.evaluate()) '${(e.widget.key! as ValueKey).value}'],
      ['pinned:b', 'pinned:a'],
    );
    expect(find.byKey(const ValueKey('unpin:a')), findsOneWidget);
    // Unpin takes a press without the pointer over it first, as a touch screen gives.
    expect(
      tester
          .widget<IgnorePointer>(
            find
                .ancestor(
                  of: find.byKey(const ValueKey('unpin:a')),
                  matching: find.byType(IgnorePointer),
                )
                .first,
          )
          .ignoring,
      isFalse,
    );
    await tester.tap(find.byKey(const ValueKey('pinned:a')));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(find.text('Pinned messages'), findsNothing);
    expect(state.direct.jump.value?.$1, 'a');
  });

  testWidgets('with nothing pinned the list says how to pin', (tester) async {
    await show(tester, page([message('1', 'Hello')]));
    await tester.tap(find.byKey(const ValueKey('direct-pins')));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(find.text('Nothing pinned yet'), findsOneWidget);
  });

  testWidgets("the friend's picture and name open the friend's card", (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-direct-card')
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..nav.page = Screen.direct
      ..contacts.friends = [
        const UiFriend(
          person: sofia,
          fingerprint: 'C07N 50S4',
          presence: UiPresence.online,
          delivered: true,
        ),
      ];
    state.direct
      ..open = sofia.id
      ..page = page([
        message('1', 'Hello', at: yesterday),
        message('2', 'How are you', mine: true),
      ], online: true);
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: Material(
            child: Builder(
              builder: (context) => Stack(
                key: shellLayerKey,
                children: [
                  const Positioned.fill(child: DirectPage()),
                  if (AppScope.of(context).nav.friendCard != null)
                    const FriendCardLayer(),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump();
    tester.takeException();
    // From the header.
    await tester.tap(find.byKey(const ValueKey('direct-friend')));
    await tester.pumpAndSettle();
    tester.takeException();
    expect(state.nav.friendCard, sofia.id);
    expect(find.byKey(const ValueKey('friend-card-message')), findsOneWidget);
    // A click beside it closes it.
    await tester.tapAt(const Offset(1400, 880));
    await tester.pumpAndSettle();
    expect(state.nav.friendCard, isNull);
    // From a message of hers: her name, then her picture.
    final name = find.descendant(
      of: find.byKey(const ValueKey('direct-item:1')),
      matching: find.text('Sofía'),
    );
    await tester.tap(name);
    await tester.pumpAndSettle();
    tester.takeException();
    expect(state.nav.friendCard, sofia.id);
    state.closeFriendCard();
    await tester.pumpAndSettle();
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('direct-item:1')),
        matching: find.byType(Avatar),
      ),
    );
    await tester.pumpAndSettle();
    tester.takeException();
    expect(state.nav.friendCard, sofia.id);
    state.closeFriendCard();
    await tester.pumpAndSettle();
    // Not from this device's own messages.
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('direct-item:2')),
        matching: find.text('Marta'),
      ),
    );
    await tester.pumpAndSettle();
    expect(state.nav.friendCard, isNull);
  });

  testWidgets(
    'a picture or thumbnail of many frames never goes past its first',
    (tester) async {
      final dir = Directory.systemTemp.createTempSync('fogata-direct-frames');
      addTearDown(() => dir.deleteSync(recursive: true));
      // 2000 by 2000 with 4000 frames of one pixel: each would be decoded whole, at once.
      final heavy = animation(2000, 4000);
      final path = '${dir.path}/sticker.gif';
      File(path).writeAsBytesSync(heavy);
      await show(
        tester,
        page([
          message(
            '1',
            '',
            files: [
              UiDirectFile(
                id: 'p',
                name: 'sticker.gif',
                size: heavy.length,
                kind: UiFileKind.picture,
                state: UiFileState.here,
                received: heavy.length,
                path: path,
              ),
            ],
          ),
          message(
            '2',
            '',
            files: [
              UiDirectFile(
                id: 'v',
                name: 'clip.mp4',
                size: 4 << 20,
                kind: UiFileKind.video,
                video: UiVideo(
                  thumb: heavy,
                  width: 1280,
                  height: 720,
                  durationMs: 4000,
                ),
                state: UiFileState.coming,
                received: 0,
              ),
            ],
          ),
        ], online: true),
      );
      // The picture is read from the disk.
      await pumpUntil(
        tester,
        () => pictureUnder(const ValueKey('direct-item:1')),
        what: 'the picture read from the disk',
      );
      tester.takeException();
      expect(
        await framesShown(tester, find.byKey(const ValueKey('direct-item:1'))),
        1,
      );
      expect(
        await framesShown(tester, find.byKey(const ValueKey('direct-item:2'))),
        1,
      );
      // More frames than any animation may have: it is not played either.
      expect(playingAnimations, 0);
    },
  );

  testWidgets('a moving picture in a chat plays by the rules of animations', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('fogata-direct-moving');
    addTearDown(() => dir.deleteSync(recursive: true));
    final moving = animation(64, 3);
    final path = '${dir.path}/wave.gif';
    File(path).writeAsBytesSync(moving);
    await show(
      tester,
      page([
        message(
          '1',
          '',
          files: [
            UiDirectFile(
              id: 'p',
              name: 'wave.gif',
              size: moving.length,
              kind: UiFileKind.picture,
              state: UiFileState.here,
              received: moving.length,
              path: path,
            ),
          ],
        ),
      ], online: true),
    );
    await pumpUntil(
      tester,
      () => playingAnimations == 1,
      what: 'the moving picture playing',
    );
    tester.takeException();
    expect(playingAnimations, 1);
    // Gone from the screen, it gives up its turn.
    await tester.pumpWidget(const SizedBox());
    expect(playingAnimations, 0);
  });

  testWidgets('Tab stays inside a picture shown large', (tester) async {
    final dir = Directory.systemTemp.createTempSync('fogata-direct-tab');
    addTearDown(() => dir.deleteSync(recursive: true));
    final path = '${dir.path}/lake.png';
    await tester.runAsync(() async {
      final recorder = ui.PictureRecorder();
      ui.Canvas(recorder).drawRect(
        const Rect.fromLTWH(0, 0, 40, 30),
        Paint()..color = const Color(0xFF2B3A67),
      );
      final image = await recorder.endRecording().toImage(40, 30);
      final png = await image.toByteData(format: ui.ImageByteFormat.png);
      File(path).writeAsBytesSync(png!.buffer.asUint8List());
    });
    final picture = UiDirectFile(
      id: 'p',
      name: 'lake.png',
      size: File(path).lengthSync(),
      kind: UiFileKind.picture,
      state: UiFileState.here,
      received: 0,
      path: path,
    );
    await show(
      tester,
      page([
        message('1', '', files: [picture]),
      ], online: true),
    );
    // The picture is read from the disk.
    await pumpUntil(
      tester,
      () => pictureUnder(const ValueKey('direct-item:1')),
      what: 'the picture read from the disk',
    );
    tester.takeException();
    await tester.tap(
      find
          .descendant(
            of: find.byKey(const ValueKey('direct-item:1')),
            matching: find.byType(Image),
          )
          .first,
    );
    await tester.pumpAndSettle();
    tester.takeException();
    // The page behind is the chat's route; the picture has a route of its own.
    final behind = ModalRoute.of(tester.element(find.byType(DirectPage)));
    final top = find.text('lake.png · ${bytes(picture.size)}');
    expect(top, findsOneWidget);
    expect(ModalRoute.of(tester.element(top)), isNot(same(behind)));
    for (var i = 0; i < 8; i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      final focused = FocusManager.instance.primaryFocus?.context;
      expect(focused, isNotNull);
      expect(
        ModalRoute.of(focused!),
        isNot(same(behind)),
        reason: 'Tab $i went to the page behind',
      );
    }
    // Esc still closes it.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(top, findsNothing);
  });

  testWidgets('the list shows unread messages and what was written last', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-direct-list')
      ..nav.page = Screen.home;
    state.direct.chats = [
      const UiDirectChat(
        away: false,
        friend: sofia,
        online: true,
        last: 'Shall we play later?',
        lastMine: false,
        lastMs: 2,
        unread: 2,
        timerMs: 0,
      ),
      UiDirectChat(
        away: false,
        friend: me,
        online: false,
        last: 'I will tell you tomorrow',
        lastMine: true,
        lastMs: 1,
        unread: 0,
        timerMs: 0,
      ),
    ];
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(
            child: SizedBox(width: 240, child: DirectList()),
          ),
        ),
      ),
    );
    expect(find.text('DIRECT MESSAGES'), findsOneWidget);
    expect(find.text('2'), findsOneWidget);
    expect(find.text('You: I will tell you tomorrow'), findsOneWidget);
    expect(state.direct.unread, 2);
  });

  testWidgets(
    'a screen reader hears whether each friend of the list is there',
    (tester) async {
      final semantics = tester.ensureSemantics();
      final state = AppState('/nonexistent/fogata-direct-presence')
        ..nav.page = Screen.home;
      UiDirectChat chat(
        UiPerson friend, {
        bool online = true,
        bool away = false,
      }) => UiDirectChat(
        away: away,
        friend: friend,
        online: online,
        last: 'Hello',
        lastMine: false,
        lastMs: 1,
        unread: 0,
        timerMs: 0,
      );
      const bea = UiPerson(
        id: 'bea',
        name: 'Bea',
        short: 'B3A0·0000',
        avatar: UiAvatar(preset: 3),
      );
      state.direct.chats = [
        chat(sofia),
        chat(me, away: true),
        chat(bea, online: false),
      ];
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(
              child: SizedBox(width: 240, child: DirectList()),
            ),
          ),
        ),
      );
      // Each row is one button, with what it says inside.
      for (final (name, said) in [
        ('Sofía', 'Online'),
        ('Marta', 'Away'),
        ('Bea', 'Offline'),
      ]) {
        expect(
          tester.getSemantics(find.text(name)).label.split('\n'),
          containsAllInOrder(['Open the chat with $name', said, name]),
        );
      }
      semantics.dispose();
    },
  );

  test('older messages are asked for one page at a time', () async {
    final state = AppState('/nonexistent/fogata-direct-more');
    state.direct
      ..open = sofia.id
      ..page = page([message('1', 'Hello')], more: true);
    final first = state.direct.more();
    for (var i = 0; i < 5; i++) {
      expect(identical(state.direct.more(), first), isTrue);
    }
    expect(state.direct.limit, 400);
    await first;
    // The page did not change here (no core): the next scroll asks for the next page.
    await state.direct.more();
    expect(state.direct.limit, 600);
    state.dispose();
  });

  testWidgets('a message that does not go stays in the box with its files', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('fogata-direct-failed');
    addTearDown(() => dir.deleteSync(recursive: true));
    final file = File('${dir.path}/plan.txt')..writeAsStringSync('plan');
    final state = await show(
      tester,
      page([message('1', 'Hello')], online: true),
    );
    await tester.runAsync(() => state.direct.addOutgoing([file.path]));
    await tester.pump();
    final box = find.descendant(
      of: find.byKey(const ValueKey('direct-message')),
      matching: find.byType(EditableText),
    );
    await tester.enterText(box, 'See you at eight');
    await tester.pump();
    // There is no core here: sending fails, as when the friend just went offline.
    await tester.tap(find.byKey(const ValueKey('direct-send')));
    await tester.pump();
    await tester.pump();
    expect(
      tester.widget<EditableText>(box).controller.text,
      'See you at eight',
    );
    expect(state.direct.waiting.map((f) => f.name), ['plan.txt']);
    expect(state.toasts.list.where((t) => t.error), isNotEmpty);
    await tester.pump(const Duration(seconds: 8));
  });

  testWidgets('files show as sending until the message goes or fails', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('fogata-direct-sending');
    addTearDown(() => dir.deleteSync(recursive: true));
    final clip = File('${dir.path}/clip.mp4')..writeAsBytesSync([0, 0, 0, 0]);
    final state = await show(
      tester,
      page([message('1', 'Hello')], online: true),
    );
    await tester.runAsync(() => state.direct.addOutgoing([clip.path]));
    await tester.pump();
    expect(find.byTooltip('Remove clip.mp4'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('direct-send')));
    await tester.pump();
    // The video is being read for its first picture: it shows as on its way, and can no
    // longer be taken out.
    expect(state.direct.waiting, isEmpty);
    expect(state.direct.sendingHere.map((f) => f.name), ['clip.mp4']);
    expect(find.text('clip.mp4'), findsOneWidget);
    expect(find.text('Sending…'), findsOneWidget);
    expect(find.byTooltip('Remove clip.mp4'), findsNothing);
    // Reading it gives up; there is no core here, so the message does not go and the file
    // waits again.
    await tester.pump(const Duration(seconds: 16));
    await tester.pump();
    expect(state.direct.sendingHere, isEmpty);
    expect(find.text('Sending…'), findsNothing);
    expect(state.direct.waiting.map((f) => f.name), ['clip.mp4']);
    expect(find.byTooltip('Remove clip.mp4'), findsOneWidget);
    await tester.pump(const Duration(seconds: 8));
  });

  testWidgets('what was written in a chat is there again on coming back', (
    tester,
  ) async {
    const leo = UiPerson(
      id: 'leo',
      name: 'Leo',
      short: 'AB3C·D4EF',
      avatar: UiAvatar(preset: 3),
    );
    final sofiaPage = page([message('1', 'Hello')], online: true);
    final state = await show(tester, sofiaPage);
    EditableText box() => tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(const ValueKey('direct-message')),
        matching: find.byType(EditableText),
      ),
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('direct-message')),
        matching: find.byType(EditableText),
      ),
      'Half a thought',
    );
    await tester.pump();
    // Another chat opens: its box is empty.
    state.direct
      ..open = leo.id
      ..page = UiDirectPage(
        away: false,
        friend: leo,
        online: true,
        connected: true,
        items: const [],
        more: false,
        pinned: const [],
        timerMs: 0,
        usage: usage,
        altered: false,
      );
    state.notifyListeners();
    await tester.pump();
    expect(box().controller.text, isEmpty);
    // Back to Sofía: the text is back.
    state.direct
      ..open = sofia.id
      ..page = sofiaPage;
    state.notifyListeners();
    await tester.pump();
    expect(box().controller.text, 'Half a thought');
  });

  testWidgets('Esc asks before taking the waiting files out', (tester) async {
    final dir = Directory.systemTemp.createTempSync('fogata-direct-esc');
    addTearDown(() => dir.deleteSync(recursive: true));
    final file = File('${dir.path}/plan.txt')..writeAsStringSync('plan');
    final state = await show(
      tester,
      page([message('1', 'Hello')], online: true),
    );
    await tester.runAsync(() => state.direct.addOutgoing([file.path]));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Remove the file?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(state.direct.waiting, hasLength(1));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Remove'));
    await tester.pumpAndSettle();
    expect(state.direct.waiting, isEmpty);
  });

  testWidgets('a pin button reached with Tab shows', (tester) async {
    await show(tester, page([message('1', 'Hello')], online: true));
    final pin = find.byKey(const ValueKey('direct-pin:1'));
    double opacity() => tester
        .widget<AnimatedOpacity>(
          find.ancestor(of: pin, matching: find.byType(AnimatedOpacity)).first,
        )
        .opacity;
    expect(opacity(), 0);
    bool focused() {
      final now = FocusManager.instance.primaryFocus?.context;
      return now != null &&
          find
              .descendant(of: pin, matching: find.byWidget(now.widget))
              .evaluate()
              .isNotEmpty;
    }

    for (var i = 0; i < 40 && !focused(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(focused(), isTrue);
    await tester.pumpAndSettle();
    expect(opacity(), 1);
  });

  testWidgets('each call line is its own, however many calls there were', (
    tester,
  ) async {
    UiDirectItem call(String id) => UiDirectItem(
      id: id,
      mine: false,
      atMs: now,
      text: '',
      files: const [],
      delivered: true,
      expiresMs: 0,
      pinned: false,
      call: const UiDirectCall(lastedMs: 60000),
    );
    await show(tester, page([call('c1'), message('1', 'Hello'), call('c2')]));
    expect(find.byKey(const ValueKey('direct-call-line:c1')), findsOneWidget);
    expect(find.byKey(const ValueKey('direct-call-line:c2')), findsOneWidget);
  });
}
