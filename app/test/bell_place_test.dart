import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/bell.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void _inside(Rect r, Size window) {
  expect(r.left, greaterThanOrEqualTo(BellLayer.margin - .5));
  expect(r.top, greaterThanOrEqualTo(BellLayer.margin - .5));
  expect(r.right, lessThanOrEqualTo(window.width - BellLayer.margin + .5));
  expect(r.bottom, lessThanOrEqualTo(window.height - BellLayer.margin + .5));
}

void main() {
  setUpAll(loadFonts);

  group('the bell\'s panel', () {
    const bell = Rect.fromLTWH(700, 6, 28, 28);

    test('hangs from the bell, right edges lined up', () {
      final r = BellLayer.place(bell, const Size(1440, 900));
      expect(r.width, BellLayer.width);
      expect(r.right, bell.right);
      expect(r.top, bell.bottom + 8);
      _inside(r, const Size(1440, 900));
    });

    test('is narrower in a window under 1000 wide', () {
      final r = BellLayer.place(bell, const Size(960, 640));
      expect(r.width, BellLayer.narrowWidth);
      expect(r.right, bell.right);
    });

    test('moves in from the left edge', () {
      final r = BellLayer.place(
        const Rect.fromLTWH(100, 6, 28, 28),
        const Size(960, 640),
      );
      expect(r.left, BellLayer.margin);
      _inside(r, const Size(960, 640));
    });

    test('fits a window narrower than itself', () {
      const window = Size(300, 400);
      final r = BellLayer.place(const Rect.fromLTWH(250, 6, 28, 28), window);
      expect(r.width, window.width - 2 * BellLayer.margin);
      _inside(r, window);
    });

    test('is no taller than the room under the bell', () {
      const window = Size(960, 300);
      final r = BellLayer.place(bell, window);
      expect(r.bottom, window.height - BellLayer.margin);
    });
  });

  for (final size in const [Size(866, 548), Size(960, 640), Size(1920, 1080)]) {
    testWidgets('the bell\'s panel opens under the bell, ${size.width}', (
      tester,
    ) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 1)
        ..nav.page = Screen.members
        ..nav.membersGroup = 'g';
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
      final bell = tester.getRect(find.byTooltip('Notifications'));
      await tester.tap(find.byTooltip('Notifications'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      final panel = tester.getRect(find.byKey(const ValueKey('bell-panel')));
      expect(
        panel.width,
        size.width < BellLayer.narrowWindow
            ? BellLayer.narrowWidth
            : BellLayer.width,
      );
      expect(panel.right, moreOrLessEquals(bell.right, epsilon: .5));
      expect(panel.top, moreOrLessEquals(bell.bottom + 8, epsilon: .5));
      _inside(panel, size);
      // Never started, so nothing to stop.
      state.callState.call = null;
    });
  }

  testWidgets('the bell\'s panel stays under the bell when the window grows', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('window_manager'),
      (call) async => false,
    );
    tester.view.physicalSize = const Size(960, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final state = fakeRoom(people: 1)
      ..nav.page = Screen.members
      ..nav.membersGroup = 'g';
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
    await tester.tap(find.byTooltip('Notifications'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    final before = tester.getRect(find.byTooltip('Notifications'));
    // Maximized.
    tester.view.physicalSize = const Size(1920, 1080);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    final bell = tester.getRect(find.byTooltip('Notifications'));
    expect(bell, isNot(before));
    final panel = tester.getRect(find.byKey(const ValueKey('bell-panel')));
    expect(panel.right, moreOrLessEquals(bell.right, epsilon: .5));
    expect(panel.top, moreOrLessEquals(bell.bottom + 8, epsilon: .5));
    _inside(panel, const Size(1920, 1080));
    state.callState.call = null;
  });
}
