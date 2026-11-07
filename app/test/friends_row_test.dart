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

  for (final width in const [960.0, 1280.0, 1920.0]) {
    testWidgets(
      'each friend\'s row has Message and Call in words, $width wide',
      (tester) async {
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          const MethodChannel('window_manager'),
          (call) async => false,
        );
        tester.view.physicalSize = Size(width, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final state = fakeRoom(people: 2)
          ..callState.call = null
          ..roomState.room = null
          ..nav.selectedGroup = null
          ..nav.page = Screen.friends
          ..contacts.friends = [
            for (final (i, presence) in [
              UiPresence.online,
              UiPresence.offline,
              UiPresence.online,
            ].indexed)
              UiFriend(
                // The longest name a profile may have.
                person: i == 2
                    ? UiPerson(
                        id: 'p2',
                        name: 'W' * 40,
                        short: 'AB2C·D2EF',
                        avatar: const UiAvatar(preset: 2),
                      )
                    : other(i),
                fingerprint: 'AB0C D0EF 9PLD 3VH$i',
                presence: presence,
                delivered: true,
              ),
          ];
        // A missed call makes Call read "Call back".
        state.notices.missedCalls['p2'] = DateTime.now().millisecondsSinceEpoch;
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
        expect(tester.takeException(), isNull);
        for (final id in ['p0', 'p1', 'p2']) {
          final message = find.byKey(ValueKey('message:$id'));
          expect(
            find.descendant(of: message, matching: find.text('Message')),
            findsOneWidget,
          );
          final row = find.byKey(ValueKey('friend-row:$id'));
          final call = find.descendant(
            of: row,
            matching: find.widgetWithText(
              FgButton,
              id == 'p2' ? 'Call back' : 'Call',
            ),
          );
          expect(call, findsOneWidget);
          // The two buttons are as tall as each other and side by side.
          final a = tester.getRect(message);
          final b = tester.getRect(call);
          expect(a.height, b.height);
          expect(a.right, lessThan(b.left));
        }
        await tester.pumpWidget(const SizedBox());
        // The toast of the friend link the test cannot make goes by itself.
        await tester.pump(const Duration(seconds: 10));
      },
    );
  }
}
