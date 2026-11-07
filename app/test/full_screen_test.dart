import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'the window leaves full screen when the room goes while it was filling the screen',
    (tester) async {
      // The window manager as the system has it: entering full screen takes a while.
      var full = false;
      final asked = <bool>[];
      final entering = Completer<void>();
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async {
          switch (call.method) {
            case 'isFullScreen':
              return full;
            case 'setFullScreen':
              final on = (call.arguments as Map)['isFullScreen'] as bool;
              asked.add(on);
              if (on) await entering.future;
              full = on;
              return null;
          }
          return false;
        },
      );
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = fakeRoom(people: 3, sharing: 1, watching: true);
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
      // The watched screen's buttons show under the pointer.
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(tester.getCenter(find.text('Leo’s screen').first));
      await tester.pump(const Duration(milliseconds: 400));
      await tester.tap(find.byTooltip('Full screen'));
      await tester.pump();
      expect(asked, [true]);
      // The room goes before the window is in full screen.
      await tester.pumpWidget(const SizedBox());
      entering.complete();
      await tester.pump();
      await tester.pump();
      expect(full, isFalse);
      expect(asked, [true, false]);
      state.callState.call = null;
    },
  );
}
