import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/call.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

Future<void> _show(
  WidgetTester tester,
  AppState state, {
  required Size size,
}) async {
  // The window follows the call's changes, as it does once a call starts.
  state.callState.call?.addListener(() => state.go(state.nav.page));
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
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
}

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

final _stream = find.byWidgetPredicate(
  (w) => w.runtimeType.toString() == '_Stream',
);

const _sizes = [
  Size(960, 640),
  Size(1280, 800),
  Size(1440, 900),
  Size(1920, 1080),
  Size(2560, 1440),
];

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

  group('chat beside the call', () {
    final show = find.byTooltip('Show the chat');
    final hide = find.byTooltip('Hide the chat');
    // The chat and files beside the call, only found while shown.
    final panel = find.byWidgetPredicate(
      (w) => w.runtimeType.toString() == '_Panel',
    );
    // A window whose room is too narrow for the chat beside the whole row of controls.
    bool small(Size size) => size.width < 1144;

    Future<void> resize(WidgetTester tester, Size size) async {
      tester.view.physicalSize = size;
      await _settle(tester);
    }

    testWidgets(
      'shown on wide windows, hidden on small ones, also while watching',
      (tester) async {
        for (final watching in [false, true]) {
          for (final size in _sizes) {
            final why = '$size${watching ? ', watching' : ''}';
            final state = fakeRoom(
              people: 3,
              sharing: watching ? 1 : 0,
              watching: watching,
            );
            await _show(tester, state, size: size);
            // Only the controls' button hides and shows it: none over the chat itself.
            expect(find.byKey(const ValueKey('hide-chat')), findsNothing);
            expect(
              panel,
              small(size) ? findsNothing : findsOneWidget,
              reason: why,
            );
            expect(
              hide,
              small(size) ? findsNothing : findsOneWidget,
              reason: why,
            );
            expect(
              show,
              small(size) ? findsOneWidget : findsNothing,
              reason: why,
            );
            // The remembered choice is not changed by a small window.
            expect(state.prefs.roomChat, isTrue, reason: why);
            expect(tester.takeException(), isNull, reason: why);
            await _done(tester);
          }
        }
      },
    );

    testWidgets(
      'its emoji picker stays inside it, and goes when the chat hides',
      (tester) async {
        final picker = find.byKey(const ValueKey('emoji-picker'));
        final button = find.byKey(const ValueKey('emoji-button'));
        for (final byHand in [false, true]) {
          final state = fakeRoom(people: 1);
          await _show(tester, state, size: const Size(1440, 900));
          await tester.tap(button);
          await _settle(tester);
          final beside = tester.getRect(panel);
          final shown = tester.getRect(picker);
          expect(shown.left, greaterThanOrEqualTo(beside.left));
          expect(shown.right, lessThanOrEqualTo(beside.right));
          expect(shown.bottom, lessThan(tester.getRect(button).top));
          if (byHand) {
            // The picker covers the controls' button: hidden from elsewhere.
            state.setRoomChat(false);
          } else {
            // A smaller window hides the chat by itself.
            await resize(tester, const Size(1000, 700));
          }
          await _settle(tester);
          expect(panel, findsNothing, reason: 'by hand: $byHand');
          expect(picker, findsNothing, reason: 'by hand: $byHand');
          // Shown again, it is as it was left: closed.
          await tester.tap(show);
          await _settle(tester);
          expect(panel, findsOneWidget);
          expect(picker, findsNothing);
          await _done(tester);
        }
      },
    );

    testWidgets('hidden and shown again by hand on wide windows', (
      tester,
    ) async {
      for (final size in _sizes.where((s) => !small(s))) {
        final state = fakeRoom(people: 3, sharing: 1, watching: true);
        await _show(tester, state, size: size);
        final beside = tester.getRect(_stream);
        await tester.tap(hide);
        await _settle(tester);
        expect(panel, findsNothing, reason: '$size');
        expect(state.prefs.roomChat, isFalse);
        expect(hide, findsNothing, reason: '$size');
        expect(show, findsOneWidget, reason: '$size');
        // The picture gets the room.
        expect(
          tester.getRect(_stream).width,
          greaterThan(beside.width),
          reason: '$size',
        );
        // A new picture keeps it hidden: only a hand brings it back.
        state.callState.call!.watch(null);
        await _settle(tester);
        state.callState.call!.watch('p0');
        await _settle(tester);
        expect(panel, findsNothing, reason: '$size: watching again');
        await tester.tap(show);
        await _settle(tester);
        expect(panel, findsOneWidget, reason: '$size');
        expect(state.prefs.roomChat, isTrue);
        await tester.tap(hide);
        await _settle(tester);
        // Kept from another page and back.
        state.goHome();
        await _settle(tester);
        state.selectGroup('g');
        await _settle(tester);
        expect(panel, findsNothing, reason: '$size: back in the room');
        expect(tester.takeException(), isNull);
        await _done(tester);
      }
    });

    testWidgets(
      'shown by hand on a small window, it stays as the window changes',
      (tester) async {
        final state = fakeRoom(people: 3, sharing: 1, watching: true);
        await _show(tester, state, size: const Size(960, 640));
        expect(panel, findsNothing);
        await tester.tap(show);
        await _settle(tester);
        expect(panel, findsOneWidget);
        // Around the size where the room stops being small, and back.
        for (final size in const [
          Size(1000, 660),
          Size(1140, 700),
          Size(1150, 700),
          Size(1180, 700),
          Size(1130, 700),
          Size(960, 640),
          Size(1600, 900),
          Size(960, 640),
        ]) {
          await resize(tester, size);
          expect(panel, findsOneWidget, reason: '$size');
        }
        expect(state.prefs.roomChat, isTrue);
        // Hidden again on the small window: wide windows keep showing it.
        await tester.tap(hide);
        await _settle(tester);
        expect(panel, findsNothing);
        expect(state.prefs.roomChat, isTrue);
        await resize(tester, const Size(1600, 900));
        expect(panel, findsOneWidget);
        await resize(tester, const Size(960, 640));
        expect(panel, findsNothing);
        expect(tester.takeException(), isNull);
        await _done(tester);
      },
    );

    testWidgets('hidden on a wide window, it stays hidden on a small one', (
      tester,
    ) async {
      final state = fakeRoom(people: 3);
      await _show(tester, state, size: const Size(1600, 900));
      await tester.tap(hide);
      await _settle(tester);
      expect(state.prefs.roomChat, isFalse);
      await resize(tester, const Size(960, 640));
      expect(panel, findsNothing);
      // Shown by hand there, it is the choice for wide windows too.
      await tester.tap(show);
      await _settle(tester);
      expect(panel, findsOneWidget);
      expect(state.prefs.roomChat, isTrue);
      await resize(tester, const Size(1600, 900));
      expect(panel, findsOneWidget);
      expect(tester.takeException(), isNull);
      await _done(tester);
    });

    testWidgets('a new call on a small window starts with the chat hidden', (
      tester,
    ) async {
      final state = fakeRoom(people: 3);
      await _show(tester, state, size: const Size(960, 640));
      await tester.tap(show);
      await _settle(tester);
      expect(panel, findsOneWidget);
      // Another call, as when another room is joined.
      state.callState.call = Call(group: 'g', me: me.id, mic: true);
      state.go(state.nav.page);
      await _settle(tester);
      expect(panel, findsNothing);
      expect(show, findsOneWidget);
      await _done(tester);
    });
    testWidgets('the chat button brings it back, with its Chat and Files', (
      tester,
    ) async {
      for (final size in const [Size(1280, 800), Size(960, 640)]) {
        final state = fakeRoom(people: 3)..prefs.roomChat = false;
        await _show(tester, state, size: size);
        expect(panel, findsNothing);
        // Only the panel has them: the group's column does not list them again.
        expect(find.text('Files'), findsNothing, reason: '$size');
        await tester.tap(show);
        await _settle(tester);
        expect(panel, findsOneWidget, reason: '$size');
        await tester.tap(find.text('Files'));
        await _settle(tester);
        expect(state.roomState.roomTab, 1, reason: '$size');
        await tester.tap(find.text('Chat'));
        await _settle(tester);
        expect(state.roomState.roomTab, 0, reason: '$size');
        await _done(tester);
      }
    });

    test('remembered with the app’s settings', () async {
      final dir = await Directory.systemTemp.createTemp('fogata-room-chat');
      addTearDown(() => dir.delete(recursive: true));
      File('${dir.path}/app.json')
          .writeAsStringSync(jsonEncode({'roomChat': false}));
      final hidden = AppState(dir.path);
      await hidden.boot();
      expect(hidden.prefs.roomChat, isFalse);
      File('${dir.path}/app.json').writeAsStringSync(jsonEncode({}));
      final shown = AppState(dir.path);
      await shown.boot();
      expect(shown.prefs.roomChat, isTrue);
    });
  });

  group('the controls under the call', () {
    // The microphone's round button as drawn: 34 points in a narrow room, 40 otherwise.
    double mic(WidgetTester tester) =>
        tester.getRect(find.byTooltip('Mute microphone').last).width;

    testWidgets('keep their size at every window size, chat shown or not', (
      tester,
    ) async {
      for (final shown in [true, false]) {
        for (final size in _sizes) {
          final state = fakeRoom(people: 3)..prefs.roomChat = shown;
          await _show(tester, state, size: size);
          expect(
            mic(tester),
            anyOf(closeTo(34, .01), closeTo(40, .01)),
            reason: '$size, chat $shown',
          );
          await _done(tester);
        }
      }
    });

    testWidgets('“Leave and close” asks without making them smaller', (
      tester,
    ) async {
      for (final size in _sizes) {
        // Alone in the room: leaving asks first.
        final state = fakeRoom(people: 1);
        await _show(tester, state, size: size);
        final before = mic(tester);
        final leave = find.byWidgetPredicate(
          (w) => w is FgButton && (w.text == 'Leave' || w.tooltip == 'Leave'),
        );
        await tester.tap(leave);
        await _settle(tester);
        final confirm = find.widgetWithText(FgButton, 'Leave and close');
        expect(confirm, findsOneWidget, reason: '$size');
        expect(
          tester.getRect(confirm).height,
          closeTo(before, .01),
          reason: '$size: the button at its own size',
        );
        // Where it fits, the other buttons stay beside it, at their size.
        if (find.byTooltip('Mute microphone').evaluate().isNotEmpty) {
          expect(mic(tester), closeTo(before, .01), reason: '$size');
        }
        expect(tester.takeException(), isNull);
        await _done(tester);
      }
    });
  });
}
