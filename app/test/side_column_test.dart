import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _show(WidgetTester tester, AppState state, Size size) async {
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
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 400));
}

Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 1));
}

double _top(String text) => find
    .descendant(of: find.byType(SideColumn), matching: find.text(text))
    .evaluate()
    .map((e) => (e.renderObject! as RenderBox).localToGlobal(Offset.zero))
    .single
    .dy;

void main() {
  setUpAll(loadFonts);

  testWidgets('a group\'s own items come before its voice rooms', (
    tester,
  ) async {
    // In its room.
    final state = fakeRoom(people: 3);
    await _show(tester, state, const Size(1280, 800));
    expect(_top('GROUP'), lessThan(_top('VOICE ROOMS')));
    expect(_top('Members'), lessThan(_top('VOICE ROOMS')));
    expect(_top('Invite'), lessThan(_top('VOICE ROOMS')));
    // The room's chat and files are in the panel beside it, not listed here again.
    for (final item in ['IN GENERAL', 'Chat', 'Files']) {
      expect(
        find.descendant(of: find.byType(SideColumn), matching: find.text(item)),
        findsNothing,
        reason: item,
      );
    }
    // Not in it.
    state
      ..callState.call = null
      ..roomState.room = null
      ..nav.page = Screen.home;
    state.notifyListeners();
    await tester.pump(const Duration(milliseconds: 400));
    expect(_top('GROUP'), lessThan(_top('VOICE ROOMS')));
    await _done(tester);
  });

  for (final size in const [Size(960, 640), Size(1920, 1080)]) {
    testWidgets('the own pill spans the rail and the column, with Settings in '
        'it, ${size.width}', (tester) async {
      final state = fakeRoom(people: 3);
      await _show(tester, state, size);
      for (final inRoom in [true, false]) {
        if (!inRoom) {
          state
            ..callState.call = null
            ..roomState.room = null
            ..nav.page = Screen.home
            ..nav.selectedGroup = null;
          state.notifyListeners();
          await tester.pump(const Duration(milliseconds: 400));
        }
        expect(tester.takeException(), isNull);
        final pill = find.byKey(const ValueKey('own-pill'));
        final box = tester.getRect(pill);
        // From near the window's left edge, under the rail, to near the column's end.
        expect(box.left, lessThanOrEqualTo(8));
        expect(box.right, greaterThan(Rail.width + SideColumn.width - 16));
        expect(box.right, lessThanOrEqualTo(Rail.width + SideColumn.width));
        expect(box.bottom, greaterThan(size.height - 16));
        // Settings in the pill, right of the headphones, and no longer in the rail.
        final settings = find.descendant(
          of: pill,
          matching: find.byTooltip('Settings'),
        );
        expect(settings, findsOneWidget);
        expect(find.byTooltip('Settings'), findsOneWidget);
        final deafen = find.descendant(
          of: pill,
          matching: find.byTooltip('Stop hearing the others'),
        );
        expect(
          tester.getRect(deafen).right,
          lessThanOrEqualTo(tester.getRect(settings).left),
        );
        if (inRoom) {
          expect(
            find.descendant(of: pill, matching: find.text('In the room')),
            findsOneWidget,
          );
        }
      }
      await tester.tap(find.byTooltip('Settings'));
      await tester.pump(const Duration(milliseconds: 400));
      expect(state.nav.settingsOpen, isTrue);
      tester.takeException();
      await _done(tester);
    });
  }

  testWidgets('without room for the column the rail keeps Settings', (
    tester,
  ) async {
    // Narrower than the window allows, as a test can make it.
    final state = fakeRoom(people: 3)
      ..callState.call = null
      ..roomState.room = null
      ..nav.page = Screen.home;
    await _show(tester, state, const Size(820, 640));
    expect(find.byType(SideColumn), findsNothing);
    expect(
      find.descendant(
        of: find.byType(Rail),
        matching: find.byTooltip('Settings'),
      ),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('own-pill')), findsNothing);
    tester.takeException();
    await _done(tester);
  });

  testWidgets('no "Fogata" over Home, at home or in a direct call', (
    tester,
  ) async {
    Finder inColumn(String text) =>
        find.descendant(of: find.byType(SideColumn), matching: find.text(text));
    final home = fakeRoom(people: 2)
      ..callState.call = null
      ..roomState.room = null
      ..roomState.roomGroup = null
      ..nav.selectedGroup = null
      ..nav.page = Screen.home;
    await _show(tester, home, const Size(1280, 800));
    expect(inColumn('Fogata'), findsNothing);
    expect(inColumn('FOGATA'), findsNothing);
    // Home is the first thing in the column, right under the title bar.
    expect(_top('Home'), lessThan(40 + 48));
    await _done(tester);

    final leo = other(0);
    final call = fakeRoom(people: 2)
      ..callState.calls = [
        UiCall(
          groupId: 'g',
          with_: leo,
          outgoing: true,
          phase: UiCallPhase.connected,
          sinceMs: DateTime.now().millisecondsSinceEpoch - 60000,
          members: [leo],
          adding: const [],
          away: const [],
        ),
      ];
    await _show(tester, call, const Size(1280, 800));
    expect(inColumn('IN THE CALL'), findsOneWidget);
    expect(inColumn('Home'), findsOneWidget);
    expect(inColumn('FOGATA'), findsNothing);
    // Never started, so nothing to stop.
    call.callState.call = null;
    await _done(tester);
  });
}
