import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/icons.dart';
import 'package:fogata/src/popover.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

Future<AppState> _show(
  WidgetTester tester,
  Size size, {
  int friends = 3,
}) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  // Not in a room, so a call would be possible: the menu still has no Call.
  final state = fakeRoom(people: 2)
    ..callState.call = null
    ..roomState.room = null
    ..roomState.roomGroup = null
    ..nav.selectedGroup = null
    ..nav.page = Screen.friends
    ..contacts.friends = [
      for (var i = 0; i < friends; i++)
        UiFriend(
          person: other(i),
          fingerprint: 'AB${i}C D${i}EF 9PLD 3VH$i',
          presence: UiPresence.online,
          delivered: true,
        ),
    ];
  await tester.pumpWidget(const SizedBox());
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
  return state;
}

Future<void> _settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
  await tester.pump();
}

/// Lets the page's toasts (no invitation can be made in a test) run out.
Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

final _menu = find.byKey(const ValueKey('friend-menu'));

bool _pressed(WidgetTester tester, String tip) =>
    tester
        .widget<FgButton>(
          find.ancestor(
            of: find.byTooltip(tip),
            matching: find.byType(FgButton),
          ),
        )
        .open ??
    false;

void _inside(Rect r, Size window) {
  expect(r.left, greaterThanOrEqualTo(popoverMargin - .5));
  expect(r.top, greaterThanOrEqualTo(popoverMargin - .5));
  expect(r.right, lessThanOrEqualTo(window.width - popoverMargin + .5));
  expect(r.bottom, lessThanOrEqualTo(window.height - popoverMargin + .5));
}

void main() {
  setUpAll(loadFonts);

  for (final size in const [Size(960, 640), Size(1920, 1080)]) {
    testWidgets('a friend\'s menu hangs from its "···", ${size.width}', (
      tester,
    ) async {
      await _show(tester, size);
      const tip = 'Options for Leo';
      final button = tester.getRect(find.byTooltip(tip));
      expect(_pressed(tester, tip), isFalse);
      await tester.tap(find.byTooltip(tip));
      await _settle(tester);
      final menu = tester.getRect(_menu);
      // Under the button, right edges lined up.
      expect(menu.top, moreOrLessEquals(button.bottom + 8, epsilon: .5));
      expect(menu.right, moreOrLessEquals(button.right, epsilon: .5));
      _inside(menu, size);
      expect(_pressed(tester, tip), isTrue);
      Finder inMenu(String text) =>
          find.descendant(of: _menu, matching: find.text(text));
      expect(inMenu('Add to “Friday games”'), findsOneWidget);
      expect(inMenu('Remove from friends…'), findsOneWidget);
      expect(
        find.descendant(of: _menu, matching: find.textContaining('Call')),
        findsNothing,
      );
      expect(
        find.descendant(of: _menu, matching: find.textContaining('Copy')),
        findsNothing,
      );
      // An icon on every row.
      for (final key in ['add:g', 'remove']) {
        expect(
          find.descendant(
            of: find.byKey(ValueKey('friend-menu:$key')),
            matching: find.byType(FgIcon),
          ),
          findsOneWidget,
        );
      }
      // Esc closes it, and the button is no longer pressed.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await _settle(tester);
      expect(_menu, findsNothing);
      expect(_pressed(tester, tip), isFalse);
      await _done(tester);
    });
  }

  testWidgets('removing a friend from the menu asks first', (tester) async {
    final state = await _show(tester, const Size(1280, 800));
    await tester.tap(find.byTooltip('Options for Sofía'));
    await _settle(tester);
    await tester.tap(find.text('Remove from friends…'));
    await _settle(tester);
    expect(_menu, findsNothing);
    expect(find.text('Remove Sofía from your friends?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await _settle(tester);
    expect(state.contacts.friends.length, 3);
    await _done(tester);
  });

  testWidgets('near the bottom of a small window, the menu stays inside it', (
    tester,
  ) async {
    const size = Size(960, 640);
    await _show(tester, size, friends: 12);
    // The lowest "···" shown whole.
    final tips = [for (var i = 0; i < 12; i++) 'Options for ${other(i).name}'];
    final shown = tips.where((t) {
      final f = find.byTooltip(t);
      if (f.evaluate().isEmpty) return false;
      final r = tester.getRect(f);
      return r.bottom <= size.height;
    }).toList();
    final lowest = shown.last;
    final button = tester.getRect(find.byTooltip(lowest));
    await tester.tap(find.byTooltip(lowest));
    await _settle(tester);
    final menu = tester.getRect(_menu);
    _inside(menu, size);
    // Over the button when there is more room above.
    if (button.top > size.height - button.bottom) {
      expect(menu.bottom, moreOrLessEquals(button.top - 8, epsilon: .5));
    }
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    await _done(tester);
  });
}
