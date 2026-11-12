// A message of nothing but line breaks (a chat takes 2000 characters, all of them may be)
// is cut after some lines with "Show more", in a room's chat and in a direct one; what was
// sent stays whole.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/direct_state.dart' show dayMs;
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/direct.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

final _spam = '${'\n' * 1999}end';

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// How tall the text of the message that ends in "end" is drawn.
double _height(WidgetTester tester) => tester
    .getSize(
      find.byWidgetPredicate(
        (w) => w is RichText && w.text.toPlainText().endsWith('end'),
      ),
    )
    .height;

/// Cut, then whole once opened, then cut again; a short message is never cut.
Future<void> _check(WidgetTester tester) async {
  expect(find.text('Show more'), findsOneWidget, reason: 'only the long one');
  final cut = _height(tester);
  expect(cut, lessThan(LongText.shownLines * 40));
  await tester.tap(find.text('Show more'));
  await tester.pump();
  expect(find.text('Show less'), findsOneWidget);
  expect(_height(tester), greaterThan(cut * 50));
  final whole = tester.widget<RichText>(
    find.byWidgetPredicate(
      (w) => w is RichText && w.text.toPlainText().endsWith('end'),
    ),
  );
  expect(whole.text.toPlainText(), _spam, reason: 'nothing of it is changed');
  await tester.ensureVisible(find.text('Show less'));
  await tester.pump();
  await tester.tap(find.text('Show less'));
  await _settle(tester);
  await tester.drag(find.text('Short\nand sweet'), const Offset(0, 100000));
  await _settle(tester);
  expect(_height(tester), cut);
}

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets('in a room', (tester) async {
    final state = fakeRoom(people: 2);
    final room = state.roomState.room!;
    UiChatLine line(String id, String text) => UiChatLine(
      id: id,
      sender: other(0).id,
      nick: other(0).name,
      text: text,
      timestampMs: DateTime.now().millisecondsSinceEpoch,
      mine: false,
    );
    state.roomState.room = UiRoom(
      startedMs: room.startedMs,
      joined: true,
      participants: room.participants,
      chat: [line('1', _spam), line('2', 'Short\nand sweet')],
      files: const [],
    );
    tester.view.physicalSize = const Size(1920, 1080);
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
    await _settle(tester);
    // Scrolled to its top, where the long one starts.
    await tester.drag(find.text('Short\nand sweet'), const Offset(0, 100000));
    await _settle(tester);
    await _check(tester);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('in a direct chat', (tester) async {
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
    final now = DateTime.now().millisecondsSinceEpoch;
    UiDirectItem message(String id, String text) => UiDirectItem(
      id: id,
      seq: 0,
      mine: false,
      atMs: now,
      text: text,
      files: const [],
      delivered: true,
      expiresMs: 0,
      pinned: false,
    );
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = AppState('/nonexistent/fogata-long-message-test')
      ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR')
      ..nav.page = Screen.direct;
    state.direct
      ..open = sofia.id
      ..page = UiDirectPage(
        away: false,
        friend: sofia,
        version: 0,
        online: false,
        lastSeenMs: now - 2 * dayMs,
        connected: false,
        items: [message('1', _spam), message('2', 'Short\nand sweet')],
        more: false,
        pinned: const [],
        timerMs: 0,
        endedMs: null,
        usage: const UiDirectUsage(
          messages: 400000,
          pictures: 0,
          videos: 0,
          files: 0,
        ),
        altered: false,
      );
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
    await tester.drag(find.text('Short\nand sweet'), const Offset(0, 100000));
    await _settle(tester);
    await _check(tester);
    await tester.pumpWidget(const SizedBox());
  });
}
