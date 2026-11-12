// Looking for words in a room's chat: only the messages with them, the words marked.

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/l10n.dart';
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

  testWidgets('a match past a long message\'s cut opens it and shows its line, '
      'until the search ends', (tester) async {
    final state = fakeRoom(people: 2);
    final room = state.roomState.room!;
    String rows(int from, int to) =>
        [for (var i = from; i < to; i++) 'row $i'].join('\n');
    state.roomState.room = UiRoom(
      startedMs: room.startedMs,
      joined: true,
      participants: room.participants,
      chat: [
        // Found in its first lines: stays cut.
        _line('early', 'needle first\n${rows(0, 60)}'),
        // Found far past the cut: opened, and the chat goes to that line.
        _line('late', '${rows(0, 250)}\nthe needle here\n${rows(250, 400)}'),
        _line('after', 'Nothing here'),
      ],
      files: const [],
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
    await _settle(tester);
    // Both cut at first.
    expect(find.text(l10n.widgetsShowMore), findsNWidgets(2));
    final search = find.descendant(
      of: find.byWidgetPredicate(
        (w) =>
            w.runtimeType.toString() == 'FgField' &&
            (w as dynamic).label == 'Search messages',
      ),
      matching: find.byType(EditableText),
    );
    await tester.enterText(search, 'needle');
    await _settle(tester);
    expect(tester.takeException(), isNull);
    // The late one shows whole, its match's line inside the chat's view.
    final marked = find.byWidgetPredicate((w) {
      if (w is! RichText) return false;
      var found = false;
      w.text.visitChildren((span) {
        if (span is TextSpan &&
            span.style?.backgroundColor != null &&
            span.text == 'needle') {
          found = true;
        }
        return !found;
      });
      return found && w.text.toPlainText().contains('the needle here');
    });
    expect(marked, findsOneWidget);
    expect(tester.widget<RichText>(marked).maxLines, isNull);
    final paragraph = tester.renderObject<RenderParagraph>(marked);
    final at = paragraph.text.toPlainText().indexOf('the needle here') + 4;
    final line = paragraph.localToGlobal(
      paragraph.getOffsetForCaret(TextPosition(offset: at), Rect.zero),
    );
    final view = tester.getRect(
      find.ancestor(of: marked, matching: find.byType(Scrollable)).first,
    );
    expect(line.dy, greaterThanOrEqualTo(view.top));
    expect(
      line.dy + paragraph.getFullHeightForCaret(TextPosition(offset: at)),
      lessThanOrEqualTo(view.bottom),
    );
    expect(find.text(l10n.widgetsShowLess), findsOneWidget);
    // Both messages count, the one with its match past the cut too.
    tester
        .state<ScrollableState>(
          find.ancestor(of: marked, matching: find.byType(Scrollable)).first,
        )
        .position
        .jumpTo(0);
    await _settle(tester);
    expect(find.text(l10n.roomMatches(2)), findsOneWidget);
    expect(_marks(tester), ['needle', 'needle']);
    // The early one stays cut: its match shows already.
    expect(find.text(l10n.widgetsShowMore), findsOneWidget);

    // Cut again by hand, it stays cut.
    await tester.ensureVisible(find.text(l10n.widgetsShowLess));
    await _settle(tester);
    await tester.tap(find.text(l10n.widgetsShowLess));
    await _settle(tester);
    expect(find.text(l10n.widgetsShowLess), findsNothing);
    expect(find.text(l10n.widgetsShowMore), findsNWidgets(2));
    await tester.ensureVisible(find.text(l10n.widgetsShowMore).last);
    await _settle(tester);
    await tester.tap(find.text(l10n.widgetsShowMore).last);
    await _settle(tester);
    expect(find.text(l10n.widgetsShowLess), findsOneWidget);

    // The search ended: cut again, as before it.
    await tester.enterText(search, '');
    await _settle(tester);
    expect(find.text(l10n.widgetsShowLess), findsNothing);
    expect(find.text(l10n.widgetsShowMore), findsNWidgets(2));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });
}
