import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

UiChatLine said(String text, int sent) => UiChatLine(
  id: text,
  sender: other(0).id,
  nick: other(0).name,
  text: text,
  timestampMs: sent,
  mine: false,
);

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'what the room held on entering shows in the order it was said and shared',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 2)
        ..roomState.roomTab = 0
        ..roomChatSmall = true;
      final group = state.roomState.roomGroup!;
      final room = state.roomState.room!;
      // Entering again: this window remembers nothing of the room, and reads it all at
      // once, the file shared between the two lines.
      state.roomState.memories.remove(group);
      final entered = UiRoom(
        startedMs: room.startedMs,
        joined: true,
        participants: room.participants,
        chat: [said('first line', 1000), said('last line', 3000)],
        files: [
          UiFile(
            sharedMs: 2000,
            id: 'f',
            name: 'middle.pdf',
            size: 1000,
            sharedBy: other(0),
            holders: [other(0)],
            mine: false,
            sent: 0,
          ),
        ],
      );
      state.roomState.room = entered;
      state.roomState.remember(group, entered);
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
      double top(String text) => tester.getTopLeft(find.text(text).first).dy;
      expect(top('first line'), lessThan(top('middle.pdf')));
      expect(top('middle.pdf'), lessThan(top('last line')));
      state.callState.call = null;
      await tester.pumpWidget(const SizedBox());
    },
  );
}
