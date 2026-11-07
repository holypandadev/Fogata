import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

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

  for (final size in const [Size(960, 640), Size(1920, 1080)]) {
    testWidgets(
      'files are shared from the chat, not the controls, ${size.width}',
      (tester) async {
        tester.view.physicalSize = size;
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
        for (final chat in [true, false]) {
          // Not awaited: saving the choice is real file work, which fake time never ends.
          unawaited(state.setRoomChat(chat));
          await tester.pump(const Duration(milliseconds: 1500));
          expect(
            find.byTooltip(chat ? 'Hide the chat' : 'Show the chat'),
            findsOneWidget,
          );
          // The pill of controls around Leave has no button for files.
          final bar = find
              .ancestor(
                of: find.byKey(const ValueKey('leave')),
                matching: find.byType(FittedBox),
              )
              .first;
          expect(
            find.byTooltip('Hide the chat'),
            chat ? findsOneWidget : findsNothing,
          );
          expect(
            find.descendant(of: bar, matching: find.byTooltip('Share files')),
            findsNothing,
          );
          expect(
            find.descendant(
              of: bar,
              matching: find.byTooltip('Turn the camera on'),
            ),
            findsOneWidget,
          );
          // The arrows beside the microphone, deafen, the camera and the screen are wide
          // enough to hit (before the pill is scaled down to fit, as all its buttons are).
          for (final tip in [
            'Microphone options',
            'Speaker options',
            'Camera options',
            'Stream quality',
          ]) {
            expect(
              tester.getSize(find.byTooltip(tip).last).width,
              greaterThanOrEqualTo(24),
              reason: tip,
            );
          }
          // The chat's own clip is there while the chat shows.
          if (chat) {
            expect(
              find.byTooltip('Share files', skipOffstage: false),
              findsOneWidget,
            );
          }
        }
        await tester.pumpWidget(const SizedBox());
        await tester.pump(const Duration(seconds: 1));
      },
    );
  }
}
