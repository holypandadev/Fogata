// Who is talking and who is typing change several times a second: the lines that show
// them follow, while the window as a whole is not told.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
  });

  testWidgets(
    'talking and typing show without the whole window building again',
    (tester) async {
      final state = fakeRoom(people: 3);
      tester.view.physicalSize = const Size(1920, 1080);
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
      for (var i = 0; i < 4; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      var told = 0;
      state.addListener(() => told++);
      expect(find.byType(SpeakingBars), findsNothing);

      // Leo talks: his tile and his row beside the room show it.
      final call = state.callState.call!;
      call.peers[other(0).id]!.speaking = true;
      call.talking.value++;
      await tester.pump();
      expect(find.byType(SpeakingBars), findsWidgets);
      call.peers[other(0).id]!.speaking = false;
      call.talking.value++;
      await tester.pump();
      expect(find.byType(SpeakingBars), findsNothing);

      // Sofía types in the room's chat.
      state.onEvent(
        UiEvent(
          kind: UiEventKind.typing,
          device: other(1).id,
          group: 'g',
          voice: AppState.firstRoom,
          received: 0,
          total: 0,
        ),
      );
      await tester.pump(const Duration(milliseconds: 400));
      expect(
        find.text('${other(1).name} is typing…', findRichText: true),
        findsOneWidget,
      );
      expect(told, 0, reason: 'the window was not told');
      await tester.pumpWidget(const SizedBox());
      // Typing goes by the wall clock, which a test's pumps do not move.
      state.dispose();
    },
  );

  testWidgets('nothing blurs what plays in a room', (tester) async {
    final state = fakeRoom(people: 3, sharing: 1, watching: true, cameras: 1);
    tester.view.physicalSize = const Size(1920, 1080);
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
    for (var i = 0; i < 4; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(find.byType(BackdropFilter), findsNothing);
    await tester.pumpWidget(const SizedBox());
    state.dispose();
  });
}
