// Looking for words in a room's chat: only the messages with them, the words marked.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

UiChatLine _line(String id, String text) => UiChatLine(
  id: id,
  sender: other(0).id,
  nick: other(0).name,
  text: text,
  timestampMs: DateTime.now().millisecondsSinceEpoch,
  mine: false,
);

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// The parts of the chat drawn marked, as a search marks them.
List<String> _marks(WidgetTester tester) => [
  for (final rich in tester.widgetList<RichText>(find.byType(RichText)))
    ...() {
      final found = <String>[];
      rich.text.visitChildren((span) {
        if (span is TextSpan && span.style?.backgroundColor != null) {
          found.add(span.text ?? '');
        }
        return true;
      });
      return found;
    }(),
];

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets('a search marks the words where they are, whatever the letters', (
    tester,
  ) async {
    final state = fakeRoom(people: 2);
    final room = state.roomState.room!;
    state.roomState.room = UiRoom(
      startedMs: room.startedMs,
      joined: true,
      participants: room.participants,
      chat: [
        _line('1', 'Caf\u00C9 abc'),
        _line('2', 'Nothing here'),
        _line('3', 'ABC and abc'),
        // Drawn without the mark that turns it around: found as it reads.
        _line('4', 'ab\u202Ec.exe'),
      ],
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
    final search = find.descendant(
      of: find.byWidgetPredicate(
        (w) =>
            w.runtimeType.toString() == 'FgField' &&
            (w as dynamic).label == 'Search messages',
      ),
      matching: find.byType(EditableText),
    );
    await tester.enterText(search, 'abc');
    await _settle(tester);
    expect(tester.takeException(), isNull);
    expect(find.text('3 matches'), findsOneWidget);
    expect(_marks(tester), ['abc', 'ABC', 'abc', 'abc']);
    expect(find.text('Nothing here', findRichText: true), findsNothing);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets('a long chat opens at its end, and a search keeps the place', (
    tester,
  ) async {
    final state = fakeRoom(people: 2);
    final room = state.roomState.room!;
    state.roomState.room = UiRoom(
      startedMs: room.startedMs,
      joined: true,
      participants: room.participants,
      chat: [for (var i = 0; i < 300; i++) _line('$i', 'Line $i')],
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
    final position = tester
        .state<ScrollableState>(
          find
              .ancestor(
                of: find.text('Line 299', findRichText: true),
                matching: find.byType(Scrollable),
              )
              .first,
        )
        .position;
    expect(position.pixels, position.maxScrollExtent);
    expect(find.text('Line 0', findRichText: true), findsNothing);

    // Further up, a search and its end leave the chat where it was.
    position.jumpTo(position.maxScrollExtent / 2);
    await _settle(tester);
    expect(find.text('Line 299', findRichText: true), findsNothing);
    final search = find.descendant(
      of: find.byWidgetPredicate(
        (w) =>
            w.runtimeType.toString() == 'FgField' &&
            (w as dynamic).label == 'Search messages',
      ),
      matching: find.byType(EditableText),
    );
    await tester.enterText(search, 'Line 29');
    await _settle(tester);
    expect(find.text('11 matches'), findsOneWidget);
    await tester.enterText(search, '');
    await _settle(tester);
    expect(find.text('Line 299', findRichText: true), findsNothing);
    expect(position.pixels, lessThan(position.maxScrollExtent - 100));
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets(
    'a chat at its cap still follows a new line taller than the one gone',
    (tester) async {
      // The core keeps the newest 500 lines: from there each new one drops the oldest, so
      // the chat has as many entries after it as before.
      const cap = 500;
      final state = fakeRoom(people: 2);
      final room = state.roomState.room!;
      UiRoom withChat(List<UiChatLine> chat) => UiRoom(
        startedMs: room.startedMs,
        joined: true,
        participants: room.participants,
        chat: chat,
        files: const [],
      );
      var chat = [for (var i = 0; i < cap; i++) _line('$i', 'Line $i')];
      state.roomState.room = withChat(chat);
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
      final position = tester
          .state<ScrollableState>(
            find
                .ancestor(
                  of: find.text('Line ${cap - 1}', findRichText: true),
                  matching: find.byType(Scrollable),
                )
                .first,
          )
          .position;
      expect(position.pixels, position.maxScrollExtent);
      for (var n = 0; n < 3; n++) {
        chat = [
          ...chat.skip(1),
          _line(
            'new$n',
            ['Taller $n', for (var k = 0; k < 8; k++) 'more'].join('\n'),
          ),
        ];
        expect(chat, hasLength(cap));
        state.roomState.room = withChat(chat);
        state.roomState.changed();
        await _settle(tester);
        expect(position.pixels, position.maxScrollExtent, reason: 'line $n');
        expect(position.extentAfter, 0, reason: 'line $n');
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 10));
    },
  );
}
