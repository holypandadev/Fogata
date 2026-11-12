import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/screens/stream_quality.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';

Future<void> show(WidgetTester tester, AppState state) async {
  tester.view.physicalSize = const Size(800, 700);
  tester.view.devicePixelRatio = 1;
  await tester.pumpWidget(
    AppScope(
      state: state,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Center(child: StreamQualityCard())),
      ),
    ),
  );
  await tester.pump();
}

void main() {
  setUpAll(loadFonts);

  testWidgets('while not sharing, nothing is said of who gets what', (
    tester,
  ) async {
    await show(tester, fakeRoom(people: 3));
    expect(find.text('GOING OUT NOW'), findsNothing);
    expect(find.text('Source'), findsOneWidget);
  });

  testWidgets('sharing, each one watching shows what they get', (tester) async {
    final state = fakeRoom(people: 4, own: true);
    final call = state.callState.call!..sharing = true;
    await show(tester, state);
    expect(find.text('GOING OUT NOW'), findsOneWidget);
    expect(find.text('Nobody is watching yet.'), findsOneWidget);

    call.screenOut.value = {
      'p0': (
        width: 1920,
        height: 1080,
        fps: 29.6,
        lowered: false,
        spared: false,
      ),
      'p1': (width: 960, height: 540, fps: 12.2, lowered: true, spared: false),
    };
    await tester.pump();
    expect(find.text('Nobody is watching yet.'), findsNothing);
    expect(find.text('Leo'), findsOneWidget);
    expect(find.text('1080p · 30 fps'), findsOneWidget);
    expect(find.text('Sofía'), findsOneWidget);
    expect(find.text('540p · 12 fps'), findsOneWidget);
    expect(
      find.text(
        "Sofía’s connection can’t take more right now, so they get a smaller "
        'picture. The others get what you chose.',
      ),
      findsOneWidget,
    );

    // Back to a good connection: no more reason to give.
    call.screenOut.value = {
      'p0': (width: 1920, height: 1080, fps: 30, lowered: false, spared: false),
      'p1': (width: 1920, height: 1080, fps: 30, lowered: false, spared: false),
    };
    await tester.pump();
    expect(find.textContaining("can’t take more"), findsNothing);
    expect(find.text('1080p · 30 fps'), findsNWidgets(2));
  });

  testWidgets('everyone watching lowered: no word of others', (tester) async {
    final state = fakeRoom(people: 2, own: true);
    final call = state.callState.call!..sharing = true;
    await show(tester, state);
    call.screenOut.value = {
      'p0': (width: 1280, height: 720, fps: 8, lowered: true, spared: false),
    };
    await tester.pump();
    expect(
      find.text(
        "Leo’s connection can’t take more right now, so they get a smaller "
        'picture.',
      ),
      findsOneWidget,
    );
  });

  testWidgets(
    'lowered for everyone to spare the computer: said so, and nobody gets what was chosen',
    (tester) async {
      final state = fakeRoom(people: 4, own: true);
      final call = state.callState.call!..sharing = true;
      await show(tester, state);
      const spared =
          'Everyone watching gets a smaller or less smooth picture right now, to '
          'spare your computer.';
      // Many watching: smaller for all, nobody's connection to blame.
      call.screenOut.value = {
        'p0': (width: 1280, height: 720, fps: 30, lowered: false, spared: true),
        'p1': (width: 1280, height: 720, fps: 29, lowered: false, spared: true),
      };
      await tester.pump();
      expect(find.text(spared), findsOneWidget);
      expect(find.textContaining("can’t take more"), findsNothing);
      expect(find.text('720p · 30 fps'), findsOneWidget);
      // And one connection that falls short too: both said, and no word of others
      // getting what was chosen, as none do.
      call.screenOut.value = {
        'p0': (width: 1280, height: 720, fps: 30, lowered: false, spared: true),
        'p1': (width: 960, height: 540, fps: 12, lowered: true, spared: true),
      };
      await tester.pump();
      expect(find.text(spared), findsOneWidget);
      expect(
        find.text(
          "Sofía’s connection can’t take more right now, so they get a smaller "
          'picture.',
        ),
        findsOneWidget,
      );
      // Back to what was chosen: nothing more to say.
      call.screenOut.value = {
        'p0': (
          width: 1920,
          height: 1080,
          fps: 30,
          lowered: false,
          spared: false,
        ),
      };
      await tester.pump();
      expect(find.text(spared), findsNothing);
    },
  );
}
