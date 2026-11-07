import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/volume.dart';

const marta = UiPerson(
  id: 'marta',
  name: 'Marta',
  short: '7Q4M·K2XR',
  avatar: UiAvatar(preset: 1),
);

void main() {
  test(
    'a voice plays from 0 to 200 % and is muted here only on request',
    () async {
      // No folder to save in: the settings are only kept in memory.
      final state = AppState('/nonexistent/fogata-volume-test');
      expect(state.gainOf('marta'), 1);
      await state.setVolume('marta', 250);
      expect(state.prefs.volumeOf('marta'), 200);
      expect(state.gainOf('marta'), 2);
      await state.setVolume('marta', -5);
      expect(state.gainOf('marta'), 0);
      await state.setVolume('marta', 140);
      await state.setSilenced('marta', true);
      expect(state.gainOf('marta'), 0);
      await state.setSilenced('marta', false);
      expect(state.gainOf('marta'), 1.4);
      // Back at 100 % nothing is kept for her.
      await state.setVolume('marta', 100);
      expect(state.prefs.volumes, isEmpty);
    },
  );

  testWidgets('a right click opens the voice menu', (tester) async {
    final state = AppState('/nonexistent/fogata-volume-test');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Scaffold(
            body: Center(
              child: VolumeTarget(
                person: marta,
                child: SizedBox(width: 200, height: 100, child: Text('Tile')),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Tile'), buttons: kSecondaryButton);
    await tester.pumpAndSettle();
    expect(find.text('Only you hear these changes'), findsOneWidget);
    expect(find.text('100%'), findsWidgets);

    // Dragging the volume to the right end.
    await tester.drag(find.byType(Slider), const Offset(400, 0));
    await tester.pumpAndSettle();
    expect(state.prefs.volumeOf('marta'), 200);

    await tester.tap(find.text('Mute for me'));
    await tester.pumpAndSettle();
    expect(state.prefs.silenced, contains('marta'));

    await tester.tap(find.text('Back to 100%'));
    await tester.pumpAndSettle();
    expect(state.prefs.volumeOf('marta'), 100);
  });
}
