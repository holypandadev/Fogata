import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// What the focus is on: the label of the control it is in, or the field's hint.
String? focused() {
  final node = FocusManager.instance.primaryFocus;
  String? label;
  node?.context?.visitAncestorElements((e) {
    final w = e.widget;
    if (w is Pressable) {
      label = w.label ?? w.tooltip ?? '?';
      return false;
    }
    if (w is FgField) {
      label = w.label ?? w.placeholder ?? 'field';
      return false;
    }
    return true;
  });
  return label;
}

Future<AppState> showHome(WidgetTester tester) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 3)
    ..nav.page = Screen.home
    ..callState.call = null;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Shell()),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 300));
  return state;
}

Future<List<String?>> tabs(WidgetTester tester, int n) async {
  final seen = <String?>[];
  for (var i = 0; i < n; i++) {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump(const Duration(milliseconds: 50));
    seen.add(focused());
  }
  return seen;
}

void main() {
  setUpAll(loadFonts);

  testWidgets('Tab reaches every part of the window and the focus stays put', (
    tester,
  ) async {
    await showHome(tester);
    final seen = await tabs(tester, 30);
    for (final label in [
      'Home',
      'Notifications',
      'Your profile',
      'Friday games',
      'Settings',
      'Stop hearing the others',
      'Add a friend',
      'Return to room',
    ]) {
      expect(seen, contains(label));
    }
    // A control given the focus keeps it while its ring is drawn.
    final at = focused();
    await tester.pump(const Duration(milliseconds: 500));
    expect(focused(), at);
  });

  testWidgets(
    'Tab goes through the header, the rail, the column, then the page',
    (tester) async {
      await showHome(tester);
      final seen = await tabs(tester, 30);
      int at(String label, {int from = 0}) => seen.indexOf(label, from);
      expect(at('Notifications'), lessThan(at('Friday games')));
      // The rail's last button before the column's first, the column's last before the
      // pill under them, and the pill's last before the page's first.
      expect(at('New group'), lessThan(at('Group settings')));
      final pill = at('Your profile');
      expect(at('Group settings'), lessThan(pill));
      expect(at('Stop hearing the others'), lessThan(at('Settings')));
      expect(at('Settings'), lessThan(at('Add a friend')));
    },
  );

  testWidgets('Tab stays in what floats over the page', (tester) async {
    final state = await showHome(tester);
    state.openGroupSettings('g');
    await tester.pump(const Duration(milliseconds: 400));
    var seen = await tabs(tester, 30);
    expect(seen, contains('Close group settings'));
    expect(seen, isNot(contains('Add a friend')));
    expect(seen, isNot(contains('Notifications')));
    state.closeGroupSettings();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.tap(find.byTooltip('Notifications'));
    await tester.pump(const Duration(milliseconds: 400));
    seen = await tabs(tester, 12);
    expect(seen, isNot(contains('Add a friend')));
    expect(seen, isNot(contains('Your profile')));
  });

  testWidgets('a click hides the focus ring and the keyboard brings it back', (
    tester,
  ) async {
    await showHome(tester);
    // The ring drawn over a control that has the focus (2 pixels wide: panels' outlines,
    // drawn over what is inside them too, are 1).
    final ring = find.byWidgetPredicate(
      (w) =>
          w is DecoratedBox &&
          w.position == DecorationPosition.foreground &&
          (w.decoration as BoxDecoration).border is Border &&
          ((w.decoration as BoxDecoration).border! as Border).top.width == 2,
    );
    await tabs(tester, 3);
    expect(ring, findsOneWidget);
    final focus = FocusManager.instance.primaryFocus;
    // A click on nothing in particular.
    await tester.tapAt(const Offset(1100, 700), kind: PointerDeviceKind.mouse);
    await tester.pump(const Duration(milliseconds: 50));
    expect(ring, findsNothing);
    // The focus stays where it was, for the keyboard to go on from there.
    expect(FocusManager.instance.primaryFocus, focus);
    // Holding Shift for a click is not using the keyboard.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump(const Duration(milliseconds: 50));
    expect(ring, findsNothing);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    // Tab there and back: the ring is on the same control again.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump(const Duration(milliseconds: 50));
    expect(FocusManager.instance.primaryFocus, focus);
    expect(ring, findsOneWidget);
  });
}
