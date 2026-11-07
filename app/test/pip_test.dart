import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/pip.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> _show(
  WidgetTester tester,
  AppState state, {
  Size size = const Size(1280, 800),
  Fg theme = Fg.darkTokens,
}) async {
  // The window follows the call's changes, as it does once a call starts.
  state.callState.call?.addListener(() => state.go(state.nav.page));
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: theme.themeData,
        home: const Material(child: Shell()),
        builder: withToasts,
      ),
    ),
  );
  await _settle(tester);
}

/// Long enough for the springs and the corner's sizes, read after each frame.
Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Ends a test: the pages' notes (the core is not there in tests) go by themselves.
Future<void> _done(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
}

final _pip = find.byKey(const ValueKey('pip'));
final _inCall = find.byKey(const ValueKey('in-call-card'));

Rect _rect(WidgetTester tester, Finder f) => tester.getRect(f);

void _expectInside(Rect r, Size window, String why) {
  expect(r.left, greaterThanOrEqualTo(-.5), reason: why);
  expect(
    r.top,
    greaterThanOrEqualTo(40 - .5),
    reason: '$why: under the header',
  );
  expect(r.right, lessThanOrEqualTo(window.width + .5), reason: why);
  expect(r.bottom, lessThanOrEqualTo(window.height + .5), reason: why);
}

void _expectApart(Rect a, Rect b, String why) {
  final overlap = a.intersect(b);
  expect(
    overlap.width > .5 && overlap.height > .5,
    isFalse,
    reason: '$why: $a and $b overlap',
  );
}

void main() {
  setUpAll(loadFonts);

  setUp(() {
    // The window's buttons ask the window manager, which tests do not have.
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  group('place', () {
    const window = Size(1280, 800);
    const picture = Size(320, 180);

    test('keeps its own shape inside its box', () {
      expect(pipSize(16 / 9), const Size(320, 180));
      expect(pipSize(4 / 3).height, 200);
      expect(pipSize(4 / 3).width, closeTo(266.7, .1));
      expect(pipSize(9 / 16), const Size(112.5, 200));
    });

    test('starts in the corner, above the corner’s cards', () {
      final bare = const PipSpot.corner().place(window, picture);
      expect(bare.right, 1280 - pipCorner);
      expect(bare.bottom, 800 - pipCorner);
      final above = const PipSpot.corner().place(
        window,
        picture,
        cards: const Size(320, 58),
      );
      expect(above.right, 1280 - pipCorner);
      expect(above.bottom, 800 - pipCorner - 58 - 12);
    });

    test('dropped past any edge, stays in the window under the header', () {
      for (final at in const [
        Offset(-900, 300),
        Offset(5000, 300),
        Offset(500, -900),
        Offset(500, 5000),
        Offset(-900, -900),
        Offset(5000, 5000),
      ]) {
        final spot = PipSpot.dropped(window, picture, at);
        final r = spot.place(window, picture);
        _expectInside(r, window, '$at');
        expect(r.left, greaterThanOrEqualTo(12), reason: '$at');
        expect(r.top, greaterThanOrEqualTo(52), reason: '$at');
        expect(r.right, lessThanOrEqualTo(1268), reason: '$at');
        expect(r.bottom, lessThanOrEqualTo(788), reason: '$at');
      }
      // The top left corner, as far as it goes.
      final corner = PipSpot.dropped(window, picture, const Offset(-900, -900));
      expect(corner.place(window, picture).topLeft, const Offset(12, 52));
    });

    test('stays where it was dropped, and inside, as the window changes', () {
      final spot = PipSpot.dropped(window, picture, const Offset(300, 200));
      expect(spot.place(window, picture).topLeft, const Offset(300, 200));
      for (final size in const [
        Size(960, 640),
        Size(1920, 1080),
        Size(400, 300),
      ]) {
        _expectInside(spot.place(size, picture), size, '$size');
      }
      // Its size changes too (another shape of picture): still inside.
      _expectInside(spot.place(window, pipSize(9 / 16)), window, 'tall');
    });

    test('dropped near the corner goes back to it', () {
      final near = const PipSpot.corner()
          .place(window, picture)
          .topLeft
          .translate(-20, 25);
      expect(PipSpot.dropped(window, picture, near).inCorner, isTrue);
      final far = near.translate(-200, 0);
      expect(PipSpot.dropped(window, picture, far).inCorner, isFalse);
    });
  });

  group('shown', () {
    testWidgets('only while the watched screen is out of sight', (
      tester,
    ) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state);
      // In the room, the room shows it.
      expect(_pip, findsNothing);
      state.goHome();
      await _settle(tester);
      expect(_pip, findsOneWidget);
      expect(find.text('Leo’s screen'), findsOneWidget);
      // In the bottom right corner, above the card of the room and clear of it.
      final pip = _rect(tester, _pip);
      final card = _rect(tester, _inCall);
      _expectApart(pip, card, 'picture and card');
      expect(pip.right, closeTo(1280 - pipCorner, .5));
      expect(pip.bottom, closeTo(card.top - 12, .5));
      expect(pip.size, const Size(320, 180));
      // Other pages too, and the settings over them.
      state.go(Screen.friends);
      await _settle(tester);
      expect(_pip, findsOneWidget);
      state.openSettings();
      await _settle(tester);
      expect(_pip, findsOneWidget);
      // The settings ask the core for what a test does not have, and say so.
      tester.takeException();
      await _done(tester);
    });

    testWidgets('over the settings opened from the room, and back', (
      tester,
    ) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state);
      state.openSettings();
      await _settle(tester);
      tester.takeException();
      expect(_pip, findsOneWidget);
      await tester.tap(_pip);
      await _settle(tester);
      expect(state.nav.settingsOpen, isFalse);
      expect(state.nav.page, Screen.room);
      expect(_pip, findsNothing);
      await _done(tester);
    });

    testWidgets('an enlarged camera floats too; this device’s own do not', (
      tester,
    ) async {
      final state = fakeRoom(people: 3, cameras: 1);
      state.callState.call!.focus('p0');
      await _show(tester, state);
      state.goHome();
      await _settle(tester);
      expect(_pip, findsOneWidget);
      expect(
        find.descendant(of: _pip, matching: find.text('Leo')),
        findsOneWidget,
      );
      await _done(tester);

      final own = fakeRoom(people: 3, own: true, ownShown: true);
      await _show(tester, own);
      own.goHome();
      await _settle(tester);
      expect(_pip, findsNothing);
      await _done(tester);
    });

    testWidgets('a click goes back to the room, where it fills the room', (
      tester,
    ) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state);
      state.goHome();
      await _settle(tester);
      await tester.tap(_pip);
      await _settle(tester);
      expect(state.nav.page, Screen.room);
      expect(state.callState.call!.watching, 'p0');
      expect(_pip, findsNothing);
      expect(find.text('Stop watching'), findsOneWidget);
      await _done(tester);
    });

    testWidgets('its button stops watching', (tester) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state);
      state.goHome();
      await _settle(tester);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: _rect(tester, _pip).center);
      await _settle(tester);
      await tester.tap(find.byTooltip('Stop watching'));
      await _settle(tester);
      expect(state.callState.call!.watching, isNull);
      expect(state.nav.page, Screen.home);
      expect(_pip, findsNothing);
      await mouse.removePointer();
      await _done(tester);
    });

    testWidgets(
      'goes when the screen ends, the person leaves or the call does',
      (tester) async {
        for (final end in ['stops sharing', 'leaves', 'call ends']) {
          final state = fakeRoom(people: 3, sharing: 1, watching: true);
          await _show(tester, state);
          state.goHome();
          await _settle(tester);
          expect(_pip, findsOneWidget, reason: end);
          final call = state.callState.call!;
          switch (end) {
            case 'stops sharing':
              call.peers['p0']!.sharing = false;
            case 'leaves':
              call.peers.remove('p0');
            default:
              state.callState.call = null;
          }
          // Whatever changes next is drawn with it.
          state.go(Screen.friends);
          await _settle(tester);
          expect(_pip, findsNothing, reason: end);
          expect(tester.takeException(), isNull, reason: end);
          await _done(tester);
        }
      },
    );
  });

  group('dragged', () {
    testWidgets('anywhere, but never out of the window', (tester) async {
      const window = Size(1280, 800);
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state, size: window);
      state.goHome();
      await _settle(tester);
      // Far past the top left corner.
      await tester.drag(
        _pip,
        const Offset(-3000, -3000),
        kind: PointerDeviceKind.mouse,
      );
      await _settle(tester);
      var pip = _rect(tester, _pip);
      expect(pip.topLeft, const Offset(12, 52));
      // Into the middle, where it stays.
      await tester.drag(
        _pip,
        const Offset(400, 250),
        kind: PointerDeviceKind.mouse,
      );
      await _settle(tester);
      pip = _rect(tester, _pip);
      expect(pip.topLeft.dx, closeTo(412, 1));
      expect(pip.topLeft.dy, closeTo(302, 1));
      // A smaller window keeps it inside; it stays put with the page under it.
      tester.view.physicalSize = const Size(960, 640);
      await _settle(tester);
      _expectInside(_rect(tester, _pip), const Size(960, 640), 'smaller');
      state.go(Screen.friends);
      await _settle(tester);
      _expectInside(_rect(tester, _pip), const Size(960, 640), 'friends');
      // Past the bottom right, it goes back to the corner above the card.
      await tester.drag(
        _pip,
        const Offset(3000, 3000),
        kind: PointerDeviceKind.mouse,
      );
      await _settle(tester);
      pip = _rect(tester, _pip);
      final card = _rect(tester, _inCall);
      expect(pip.right, closeTo(960 - pipCorner, .5));
      expect(pip.bottom, closeTo(card.top - 12, .5));
      expect(tester.takeException(), isNull);
      await _done(tester);
    });

    testWidgets('the toasts keep clear of it and of the card', (tester) async {
      for (final size in const [Size(960, 640), Size(1280, 800)]) {
        final state = fakeRoom(people: 3, sharing: 1, watching: true);
        await _show(tester, state, size: size);
        state.goHome();
        await _settle(tester);
        // One toast, then three, which rise higher.
        state.say('Before');
        await _settle(tester);
        _expectApart(
          _rect(tester, find.text('Before')),
          _rect(tester, _inCall),
          '$size: toast and card',
        );
        state.say(
          'A long note that takes most of the width a toast may take here',
        );
        state.say('Another one');
        await _settle(tester);
        final pip = _rect(tester, _pip);
        final card = _rect(tester, _inCall);
        for (final text in [
          'A long note that takes most of the width a toast may take here',
          'Another one',
        ]) {
          final toast = _rect(tester, find.text(text));
          _expectApart(toast, pip, '$size: toast and picture');
          _expectApart(toast, card, '$size: toast and card');
          // Nor over the side column and this device's card at its bottom.
          expect(toast.left, greaterThanOrEqualTo(312), reason: '$size');
        }
        await _done(tester);
      }
    });

    testWidgets(
      'with nothing in the corner, the toasts keep off the side column',
      (tester) async {
        for (final size in const [Size(960, 640), Size(1440, 900)]) {
          // In the room, with no floating picture and no card in the corner.
          final state = fakeRoom(people: 3);
          await _show(tester, state, size: size);
          expect(_inCall, findsNothing);
          const long =
              'A long note that takes most of the width a toast may take here';
          state.say(long);
          await _settle(tester);
          final toast = _rect(tester, find.text(long));
          expect(toast.left, greaterThanOrEqualTo(312), reason: '$size');
          _expectApart(
            toast,
            _rect(tester, find.text('In the room')),
            '$size: toast and this device’s card',
          );
          await _done(tester);
        }
      },
    );

    testWidgets('a preview of one size: its edges move it, nothing resizes it', (
      tester,
    ) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state);
      state.goHome();
      await _settle(tester);
      final size = _rect(tester, _pip).size;
      expect(size, pipSize(16 / 9));
      // Pulled by its right edge, it moves whole (far enough not to go back to
      // the corner).
      final before = _rect(tester, _pip);
      await tester.dragFrom(
        before.centerRight - const Offset(3, 0),
        const Offset(-500, -320),
        kind: PointerDeviceKind.mouse,
      );
      await _settle(tester);
      final moved = _rect(tester, _pip);
      expect(moved.size, size);
      expect(moved.topLeft, isNot(before.topLeft));
      // No pointer over it, edges and corners included, shows a resize cursor.
      final r = moved;
      // The drag left the mouse over the window: it moves over the picture.
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      for (final at in [
        r.center,
        r.topLeft + const Offset(1, 1),
        r.bottomRight - const Offset(1, 1),
        r.centerRight - const Offset(1, 0),
        r.bottomCenter - const Offset(0, 1),
      ]) {
        await mouse.moveTo(at);
        await tester.pump();
        expect(
          RendererBinding.instance.mouseTracker.debugDeviceActiveCursor(1),
          SystemMouseCursors.click,
          reason: '$at',
        );
      }
      // A pinch on a trackpad or a touch screen does not change it either.
      final center = moved.center;
      final a = await tester.startGesture(center - const Offset(20, 0));
      final b = await tester.startGesture(center + const Offset(20, 0));
      await a.moveBy(const Offset(-60, 0));
      await b.moveBy(const Offset(60, 0));
      await a.up();
      await b.up();
      await _settle(tester);
      void same(String why) {
        final now = _rect(tester, _pip).size;
        expect(now.width, closeTo(size.width, .01), reason: why);
        expect(now.height, closeTo(size.height, .01), reason: why);
      }

      same('pinched');
      // Nor does the window's size.
      for (final window in const [Size(960, 640), Size(2560, 1440)]) {
        tester.view.physicalSize = window;
        await _settle(tester);
        same('$window');
      }
      expect(tester.takeException(), isNull);
      await _done(tester);
    });

    testWidgets('in light theme too', (tester) async {
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
      await _show(tester, state, theme: Fg.lightTokens);
      state.go(Screen.friends);
      await _settle(tester);
      expect(_pip, findsOneWidget);
      expect(tester.takeException(), isNull);
      await _done(tester);
    });
  });
}
