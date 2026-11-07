import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

const long = 'Training plan season 2026-2027 final version (2) revised.pdf';

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'a long file name in Files shows on two lines and whole on hover',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      // A small window: the chat and files shown by hand.
      final state = fakeRoom(people: 2)
        ..roomState.roomTab = 1
        ..roomChatSmall = true;
      final room = state.roomState.room!;
      state.roomState.room = UiRoom(
        startedMs: room.startedMs,
        joined: true,
        participants: room.participants,
        chat: const [],
        files: [
          UiFile(
            id: 'f',
            name: long,
            size: 18400000,
            sharedBy: other(0),
            holders: [other(0)],
            mine: false,
            sent: 0,
          ),
        ],
      );
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: Shell()),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 600));
      expect(tester.takeException(), isNull);
      final name = find.text(long);
      expect(name, findsOneWidget);
      // Two lines of 13 px text.
      expect(tester.getSize(name).height, greaterThan(30));
      expect(find.byTooltip(long), findsOneWidget);
      state.callState.call = null;
      await tester.pumpWidget(const SizedBox());
    },
  );
}
