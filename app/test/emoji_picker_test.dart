import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/emoji.dart';
import 'package:fogata/src/l10n.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/emoji_picker.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart' show loadFonts, me;

/// A message box with the emoji button and ":name" completion, as the chats have.
class _Box extends StatefulWidget {
  const _Box();

  @override
  State<_Box> createState() => _BoxState();
}

class _BoxState extends State<_Box> {
  final text = TextEditingController();
  final focus = FocusNode();
  final sent = <String>[];

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.bottomRight,
    child: SizedBox(
      width: 340,
      child: Row(
        children: [
          Expanded(
            child: EmojiComplete(
              controller: text,
              max: 20,
              child: FgField(
                key: const ValueKey('box'),
                controller: text,
                focusNode: focus,
                onSubmitted: sent.add,
              ),
            ),
          ),
          EmojiButton(controller: text, focusNode: focus, max: 20),
        ],
      ),
    ),
  );
}

/// A chat column 340 wide at the window's right, its message box at the bottom, as the
/// room's chat beside the call; it can slide away ([shown]) or go offstage ([visible]).
class _Column extends StatefulWidget {
  const _Column();

  @override
  State<_Column> createState() => _ColumnState();
}

class _ColumnState extends State<_Column> {
  final text = TextEditingController();
  final focus = FocusNode();
  bool shown = true;
  bool visible = true;

  void set({bool? shown, bool? visible}) => setState(() {
    this.shown = shown ?? this.shown;
    this.visible = visible ?? this.visible;
  });

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.centerRight,
    child: Visibility(
      visible: visible,
      maintainState: true,
      child: PickerArea(
        shown: shown,
        child: SizedBox(
          key: const ValueKey('column'),
          width: 340,
          child: Column(
            children: [
              const Spacer(),
              Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Expanded(
                      child: EmojiComplete(
                        controller: text,
                        child: FgField(
                          key: const ValueKey('box'),
                          controller: text,
                          focusNode: focus,
                        ),
                      ),
                    ),
                    EmojiButton(controller: text, focusNode: focus),
                    // The send button's place, right of the emoji button.
                    const SizedBox(width: 40),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

Future<_ColumnState> _showColumn(WidgetTester tester, Size size) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState('/nonexistent/fogata-emoji-test')
    ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR');
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: _Column()),
      ),
    ),
  );
  return tester.state<_ColumnState>(find.byType(_Column));
}

Rect _rect(WidgetTester tester, Finder finder) => tester.getRect(finder);

Future<(AppState, _BoxState)> _show(WidgetTester tester) async {
  tester.view.physicalSize = const Size(960, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState('/nonexistent/fogata-emoji-test')
    ..contacts.profile = const UiProfile(person: me, fingerprint: '7Q4M K2XR');
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: _Box()),
      ),
    ),
  );
  return (state, tester.state<_BoxState>(find.byType(_Box)));
}

Future<void> _open(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('emoji-button')));
  await tester.pump(const Duration(milliseconds: 400));
  expect(find.byKey(const ValueKey('emoji-picker')), findsOneWidget);
}

void main() {
  setUpAll(() async {
    await loadFonts();
    // The English words, read outside a test's fake clock, which would never finish it.
    await loadEmojiWords();
  });

  test(
    'the table has every group, and search finds by name and short name',
    () {
      final all = allEmoji;
      expect(all.length, greaterThan(1500));
      for (final group in EmojiGroup.values) {
        expect(
          all.where((e) => e.group == group),
          isNotEmpty,
          reason: '$group',
        );
      }
      expect(all.first.text, '😀');
      expect(searchEmoji('thumbs').first.text, '👍');
      expect(searchEmoji('+1').first.text, '👍');
      expect(searchEmoji(':thumbsup').first.text, '👍');
      expect(searchEmoji('heart').first.text, '❤️');
      expect(searchEmoji('tears of').first.text, '😂');
      expect(searchEmoji('zzzzqq'), isEmpty);
      // Among equals, the one used lately comes first.
      expect(searchEmoji('grinning').first.text, '😀');
      expect(searchEmoji('grinning', recent: ['😸']).first.text, '😸');
    },
  );

  test(
    'skin tones go after the first code point, in place of its selector',
    () {
      final thumbs = emojiOf('👍')!;
      expect(thumbs.tones, isTrue);
      expect(thumbs.toned(0), '👍');
      expect(thumbs.toned(3), '👍🏽');
      // Index pointing up is written with a presentation selector.
      final up = emojiOf('☝️')!;
      expect(up.toned(5), '☝🏿');
      // A person at a computer: the tone goes on the person.
      final coder = allEmoji.firstWhere((e) => e.name == 'woman technologist');
      expect(coder.toned(2), '👩🏼‍💻');
      expect(emojiOf('😀')!.toned(4), '😀');
      // Handshake's tones came after the version the table stops at.
      expect(emojiOf('🤝')!.tones, isFalse);
    },
  );

  test('short names between colons', () {
    expect(shortName(emojiOf('❤️')!), 'red_heart');
    expect(emojiNamed('red_heart')!.text, '❤️');
    expect(emojiNamed('joy')!.text, '😂');
    expect(emojiNamed('nothing_like_this'), isNull);
  });

  test(
    'symbols asking for their emoji form get the colour font in messages',
    () {
      const style = TextStyle(fontFamily: 'Inter');
      expect(emojiText('plain ❤ text', style).children, isNull);
      final span = emojiText('I ❤️ it, ❤️‍🔥 and 1️⃣ 😀', style);
      final parts = [for (final c in span.children!) (c as TextSpan).text];
      expect(parts, ['I ', '❤️', ' it, ', '❤️‍🔥', ' and ', '1️⃣', ' 😀']);
      expect((span.children![1] as TextSpan).style!.fontFamily, emojiFont);
      expect(span.toPlainText(), 'I ❤️ it, ❤️‍🔥 and 1️⃣ 😀');
    },
  );

  test(
    'an emoji goes at the cursor, in place of the selection, within the limit',
    () {
      final c = TextEditingController(text: 'hello world');
      c.selection = const TextSelection.collapsed(offset: 5);
      expect(insertAtCursor(c, '👋'), isTrue);
      expect(c.text, 'hello👋 world');
      expect(c.selection.baseOffset, 5 + '👋'.length);
      c.selection = const TextSelection(baseOffset: 0, extentOffset: 5);
      insertAtCursor(c, '🙂');
      expect(c.text, '🙂👋 world');
      expect(insertAtCursor(c, '🎉', max: 8), isFalse);
      expect(c.text, '🙂👋 world');
    },
  );

  testWidgets(
    'search, Enter puts the emoji at the cursor and closes the picker',
    (tester) async {
      await loadFonts();
      final (state, box) = await _show(tester);
      box.text.text = 'ab';
      box.text.selection = const TextSelection.collapsed(offset: 1);
      await _open(tester);
      await tester.enterText(
        find.byKey(const ValueKey('emoji-search')),
        'thumbs',
      );
      await tester.pump();
      expect(find.text('Thumbs up'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('emoji-picker')), findsNothing);
      expect(box.text.text, 'a👍b');
      expect(box.sent, isEmpty);
      expect(state.prefs.recentEmoji.first, '👍');
      expect(box.focus.hasFocus, isTrue);
      // Used once, it is first among the recently used.
      await _open(tester);
      expect(find.text('Recently used'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('emoji-picker')), findsNothing);
      expect(box.text.text, 'a👍b');
    },
  );

  testWidgets('arrows choose in the grid, and a click picks', (tester) async {
    final (_, box) = await _show(tester);
    await _open(tester);
    // Two right and one down: the eleventh emoji of the first group.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    final chosen = allEmoji[10];
    final name = tester.widget<Text>(find.byKey(const ValueKey('emoji-name')));
    expect(name.data!.toLowerCase(), chosen.name);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump(const Duration(milliseconds: 400));
    expect(box.text.text, chosen.text);
    // A click on another one.
    await _open(tester);
    await tester.tap(find.text('😃'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(box.text.text, '${chosen.text}😃');
  });

  testWidgets('the skin tone chosen is kept and applies to what is picked', (
    tester,
  ) async {
    final (state, box) = await _show(tester);
    await _open(tester);
    await tester.tap(find.byKey(const ValueKey('emoji-tone')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byKey(const ValueKey('tone-3')));
    await tester.pump(const Duration(milliseconds: 400));
    expect(state.prefs.emojiTone, 3);
    await tester.enterText(
      find.byKey(const ValueKey('emoji-search')),
      'waving',
    );
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump(const Duration(milliseconds: 400));
    expect(box.text.text, '👋🏽');
    // Kept as its default form among the recently used.
    expect(state.prefs.recentEmoji.first, '👋');
  });

  testWidgets(
    'in a narrow column the picker fits inside it, above its button',
    (tester) async {
      for (final size in [const Size(1440, 900), const Size(960, 640)]) {
        await _showColumn(tester, size);
        await _open(tester);
        // Its spring is done.
        await tester.pump(const Duration(milliseconds: 400));
        final picker = _rect(
          tester,
          find.byKey(const ValueKey('emoji-picker')),
        );
        final column = _rect(tester, find.byKey(const ValueKey('column')));
        final button = _rect(
          tester,
          find.byKey(const ValueKey('emoji-button')),
        );
        // Fewer emoji to a line, so it fits with room to spare on both sides.
        expect(picker.width, lessThan(column.width - 16), reason: '$size');
        expect(picker.left, greaterThanOrEqualTo(column.left + 8 - .5));
        expect(picker.right, lessThanOrEqualTo(column.right - 8 + .5));
        expect(picker.top, greaterThanOrEqualTo(8 - .5));
        expect(picker.bottom, closeTo(button.top - 8, .5), reason: '$size');
        expect(picker.right, greaterThanOrEqualTo(button.right - .5));
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump(const Duration(milliseconds: 400));
        expect(find.byKey(const ValueKey('emoji-picker')), findsNothing);
      }
    },
  );

  testWidgets('the picker and ":name" close when their column goes', (
    tester,
  ) async {
    final column = await _showColumn(tester, const Size(1440, 900));
    final picker = find.byKey(const ValueKey('emoji-picker'));
    final complete = find.byKey(const ValueKey('emoji-complete'));
    // The column starts to slide away.
    await _open(tester);
    column.set(shown: false);
    await tester.pump();
    expect(picker, findsNothing);
    await tester.pump();
    column.set(shown: true);
    await tester.pump(const Duration(milliseconds: 400));
    expect(picker, findsNothing);
    // Offstage, as a hidden part of the page.
    await _open(tester);
    column.set(visible: false);
    await tester.pump();
    expect(picker, findsNothing);
    column.set(visible: true);
    await tester.pump(const Duration(milliseconds: 400));
    expect(picker, findsNothing);
    // The suggestions for ":name" too, and they do not come back while it is away.
    await tester.tap(find.byKey(const ValueKey('box')));
    await tester.enterText(find.byKey(const ValueKey('box')), 'ok :thumbs');
    await tester.pump(const Duration(milliseconds: 400));
    expect(complete, findsOneWidget);
    column.set(shown: false);
    await tester.pump();
    await tester.pump();
    expect(complete, findsNothing);
    column.text.text = 'ok :fire';
    await tester.pump(const Duration(milliseconds: 400));
    expect(complete, findsNothing);
    // Back in view, the button opens it again.
    column.set(shown: true);
    await tester.pump(const Duration(milliseconds: 400));
    await _open(tester);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets(
    '":name" offers emoji above the box; Enter takes one, not the message',
    (tester) async {
      final (_, box) = await _show(tester);
      await tester.tap(find.byKey(const ValueKey('box')));
      await tester.enterText(find.byKey(const ValueKey('box')), 'ok :thumbs');
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('emoji-complete')), findsOneWidget);
      expect(find.text(':thumbs_up:'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump(const Duration(milliseconds: 400));
      expect(box.text.text, 'ok 👍');
      expect(box.sent, isEmpty);
      expect(find.byKey(const ValueKey('emoji-complete')), findsNothing);
      // Esc leaves the text as it is.
      await tester.enterText(find.byKey(const ValueKey('box')), 'ok :fir');
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('emoji-complete')), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const ValueKey('emoji-complete')), findsNothing);
      expect(box.text.text, 'ok :fir');
      // A whole known name between colons turns into its emoji; a time does not.
      await tester.enterText(find.byKey(const ValueKey('box')), 'ha :joy:');
      await tester.pump();
      expect(box.text.text, 'ha 😂');
      await tester.enterText(find.byKey(const ValueKey('box')), 'at 10:30:');
      await tester.pump();
      expect(box.text.text, 'at 10:30:');
      expect(find.byKey(const ValueKey('emoji-complete')), findsNothing);
    },
  );

  testWidgets('":name" and the picker search use the language in use', (
    tester,
  ) async {
    addTearDown(() => useLanguage('en'));
    useLanguage('es');
    await tester.runAsync(loadEmojiWords);
    final (_, box) = await _show(tester);
    // The app state starts in its own language.
    useLanguage('es');
    await tester.tap(find.byKey(const ValueKey('box')));
    await tester.enterText(find.byKey(const ValueKey('box')), 'te :corazón');
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byKey(const ValueKey('emoji-complete')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump(const Duration(milliseconds: 400));
    expect(box.text.text, 'te ❤️');
  });

  testWidgets(
    'the lines are laid out once, not again as the pointer crosses the emoji',
    (tester) async {
      await _show(tester);
      final before = emojiLayouts;
      await _open(tester);
      expect(emojiLayouts, before + 1);
      // The pointer goes over several emoji: each builds the picker again, with the
      // name of the one under it.
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      addTearDown(mouse.removePointer);
      for (final text in ['😀', '😃', '😄', '😁']) {
        // The emoji in the grid, not the smaller one by its name.
        final cell = find.byWidgetPredicate(
          (w) => w is Text && w.data == text && w.style?.fontSize == 24,
        );
        await mouse.moveTo(tester.getCenter(cell));
        await tester.pump();
      }
      // The name shown is the last one's: the pointer was followed.
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('emoji-name')))
            .data!
            .toLowerCase(),
        emojiOf('😁')!.name,
      );
      expect(emojiLayouts, before + 1);
      // A search lays them out again, once.
      await tester.enterText(find.byKey(const ValueKey('emoji-search')), 'cat');
      await tester.pump();
      await tester.pump();
      expect(emojiLayouts, before + 2);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump(const Duration(milliseconds: 400));
    },
  );
}
