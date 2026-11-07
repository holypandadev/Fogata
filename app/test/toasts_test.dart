import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/settings.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

/// Whether [finder]'s widget is drawn after (over) [under]'s.
bool _over(Finder finder, Finder under) {
  final top = finder.evaluate().single.renderObject!;
  final below = under.evaluate().first.renderObject!;
  var shown = false;
  var after = false;
  void visit(RenderObject o) {
    if (identical(o, below)) shown = true;
    if (identical(o, top)) after = shown;
    o.visitChildren(visit);
  }

  top.owner!.rootNode!.visitChildren(visit);
  return after;
}

void main() {
  setUpAll(loadFonts);

  testWidgets('toasts show over the settings and over a dialog', (
    tester,
  ) async {
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
      ..nav.page = Screen.home;
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Shell()),
          builder: withToasts,
        ),
      ),
    );
    state.openSettings();
    state.showSettingsSection(SettingsSection.profile);
    await tester.pump(const Duration(milliseconds: 600));
    tester.takeException();

    // Said from inside the settings: the toast is drawn over them.
    state.say('Animated pictures can be up to 550 KB; this one is 700 KB.');
    await tester.pump(const Duration(milliseconds: 600));
    final said = find.text(
      'Animated pictures can be up to 550 KB; this one is 700 KB.',
    );
    expect(said, findsOneWidget);
    expect(
      find.ancestor(of: said, matching: find.byType(Navigator)),
      findsNothing,
    );
    expect(_over(said, find.byType(Settings)), isTrue);

    // And over a dialog opened on top of everything.
    showFgDialog<void>(
      tester.element(find.byType(Settings)),
      builder: (context) => const Text('A dialog'),
    );
    await tester.pump(const Duration(milliseconds: 400));
    expect(_over(said, find.text('A dialog')), isTrue);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 10));
  });

  testWidgets(
    'a toast about a saved file copies its path; others let clicks through',
    (tester) async {
      String? copied;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            copied = (call.arguments as Map)['text'] as String?;
          }
          return null;
        },
      );
      final state = AppState('/nonexistent/fogata-toasts-test');
      await tester.pumpWidget(
        AppScope(
          state: state,
          child: MaterialApp(
            theme: Fg.darkTokens.themeData,
            home: const Material(child: SizedBox.expand()),
            builder: withToasts,
          ),
        ),
      );
      state.say('Nothing to do here.');
      state.say(
        'Saved to /home/ana/Downloads/plan.pdf',
        path: '/home/ana/Downloads/plan.pdf',
      );
      await tester.pump(const Duration(milliseconds: 600));
      bool ignored(String text) => tester
          .widget<IgnorePointer>(
            find
                .ancestor(
                  of: find.text(text),
                  matching: find.byType(IgnorePointer),
                )
                .first,
          )
          .ignoring;
      expect(ignored('Nothing to do here.'), isTrue);
      expect(ignored('Saved to /home/ana/Downloads/plan.pdf'), isFalse);
      expect(find.text('Show in folder'), findsOneWidget);
      await tester.tap(find.text('Copy path'));
      await tester.pump();
      expect(copied, '/home/ana/Downloads/plan.pdf');
      expect(find.text('Copied'), findsOneWidget);
      // It stays longer than a toast with nothing to do, then goes too.
      await tester.pump(const Duration(seconds: 5));
      expect(find.text('Nothing to do here.'), findsNothing);
      expect(find.text('Copied'), findsOneWidget);
      await tester.pump(const Duration(seconds: 6));
      expect(find.text('Copied'), findsNothing);
    },
  );
}
