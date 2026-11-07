// ignore_for_file: invalid_use_of_protected_member

import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/shown.dart';

void main() {
  // testWidgets fails when a timer is still pending at its end: a toast's timer that
  // outlived its window would.
  testWidgets(
    'states let go of leave no toast timers and no watch on the window behind',
    (tester) async {
      expect(windowShown.hasListeners, isFalse);
      for (var i = 0; i < 3; i++) {
        final state = AppState('/nonexistent/fogata-dispose-test-$i');
        // The parts that watch whether the window shows.
        expect(state.direct.voice.recordingFor, isNull);
        expect(windowShown.hasListeners, isTrue);
        state.say('one');
        state.say('with a file', path: '/nonexistent/file');
        state.say('wrong', error: true);
        await tester.pump(const Duration(seconds: 5));
        // The first is leaving: its own timer to go is running.
        state.dispose();
        expect(windowShown.hasListeners, isFalse);
      }
    },
  );
}
