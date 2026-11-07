import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/user_panel.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart' show loadFonts, me;

void main() {
  setUpAll(loadFonts);

  testWidgets(
    'the person’s panel stays over its pill when the window changes size',
    (tester) async {
      // The window's buttons ask the window manager, which tests do not have.
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final state = AppState('/nonexistent/fogata-anchored-test')
        ..contacts.profile = const UiProfile(
          person: me,
          fingerprint: '7Q4M K2XR',
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
      // Its picture and name in the pill at the bottom left.
      await tester.tap(find.text(me.name));
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(UserPanelLayer), findsOneWidget);
      Rect panel() => tester.getRect(
        find
            .descendant(
              of: find.byType(UserPanelLayer),
              matching: find.byType(TweenAnimationBuilder<double>),
            )
            .first,
      );
      Rect pill() => tester.getRect(find.text(me.name).first);
      final before = panel();
      expect(panel().bottom, lessThan(pill().top));
      // Something else changing keeps it.
      state.goHome();
      await tester.pump(const Duration(milliseconds: 100));
      expect(state.nav.userPanelFrom, isNotNull);
      tester.view.physicalSize = const Size(1440, 900);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.byType(UserPanelLayer), findsOneWidget);
      expect(panel().top, isNot(before.top));
      expect(panel().bottom, lessThan(pill().top));
      expect(pill().top - panel().bottom, lessThan(40));
    },
  );
}
