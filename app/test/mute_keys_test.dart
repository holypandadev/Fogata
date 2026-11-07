import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/host.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<AppState> _show(WidgetTester tester) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = fakeRoom(people: 2)
    ..callState.call = null
    ..roomState.room = null
    ..roomState.roomGroup = null
    ..nav.selectedGroup = null
    ..nav.page = Screen.home
    ..prefs.joinWithMic = true;
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

/// Presses M with [modifiers] held, and lets them go.
Future<void> _press(
  WidgetTester tester,
  List<LogicalKeyboardKey> modifiers,
) async {
  for (final key in modifiers) {
    await tester.sendKeyDownEvent(key);
  }
  await tester.sendKeyEvent(LogicalKeyboardKey.keyM);
  for (final key in modifiers.reversed) {
    await tester.sendKeyUpEvent(key);
  }
  await tester.pump();
}

void main() {
  setUpAll(loadFonts);
  const ctrl = LogicalKeyboardKey.controlLeft;
  const cmd = LogicalKeyboardKey.metaLeft;
  const shift = LogicalKeyboardKey.shiftLeft;

  testWidgets(
    'on macOS the microphone is ⌘⇧M, as its tooltip says, not Ctrl Shift M',
    (tester) async {
      pinnedHost = HostSystem.macos;
      addTearDown(() => pinnedHost = HostSystem.linux);
      final state = await _show(tester);
      expect(find.byTooltip(RegExp(r'⌘⇧M')), findsOneWidget);
      expect(find.byTooltip(RegExp(r'Ctrl')), findsNothing);
      await _press(tester, [ctrl, shift]);
      expect(state.prefs.joinWithMic, isTrue);
      await _press(tester, [cmd, shift]);
      expect(state.prefs.joinWithMic, isFalse);
      await _press(tester, [cmd, shift]);
      expect(state.prefs.joinWithMic, isTrue);
      await tester.pumpWidget(const SizedBox());
    },
  );

  testWidgets(
    'elsewhere it is Ctrl Shift M, and ⌘ (the Super key) does nothing',
    (tester) async {
      final state = await _show(tester);
      expect(find.byTooltip(RegExp(r'Ctrl\+Shift\+M')), findsOneWidget);
      await _press(tester, [cmd, shift]);
      expect(state.prefs.joinWithMic, isTrue);
      await _press(tester, [ctrl, shift]);
      expect(state.prefs.joinWithMic, isFalse);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
