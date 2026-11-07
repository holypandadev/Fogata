import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  for (final width in const [960.0, 1280.0, 1920.0]) {
    testWidgets('a room\'s title bar has no chip about it being ephemeral, '
        '$width wide', (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = Size(width, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 3);
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.textContaining('Ephemeral'), findsNothing);
      expect(
        find.textContaining('gone when the last one leaves'),
        findsNothing,
      );
      // How the room is reached stays, where the bar is wide enough for it.
      expect(
        find.text('Direct · encrypted'),
        width >= 1180 ? findsOneWidget : findsNothing,
      );
      // The button that opens the quick switcher is there, as on every other page.
      expect(find.byKey(const ValueKey('find')), findsOneWidget);
      // The chat still says the room's chat goes with it.
      if (state.roomChatShown) {
        expect(
          find.text('This chat disappears when the room closes'),
          findsOneWidget,
        );
      }
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 1));
    });
  }
}
