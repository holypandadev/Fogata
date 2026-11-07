import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/devices.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

void main() {
  test('noise suppression is off until turned on, and read back', () async {
    final dir = await Directory.systemTemp.createTemp('fogata-ns-test');
    addTearDown(() => dir.delete(recursive: true));
    final fresh = AppState(dir.path);
    await fresh.boot();
    expect(fresh.prefs.noiseSuppression, isFalse);

    await File('${dir.path}/app.json')
        .writeAsString('{"noiseSuppression": true}');
    final kept = AppState(dir.path);
    await kept.boot();
    expect(kept.prefs.noiseSuppression, isTrue);
  });

  testWidgets('the switch sits under the microphone and turns it on and off', (
    tester,
  ) async {
    final state = AppState('/nonexistent/fogata-ns-test');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Scaffold(body: DeviceChoices()),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final mic = tester.getTopLeft(find.text('Microphone'));
    final label = tester.getTopLeft(find.text('Noise suppression'));
    final speaker = tester.getTopLeft(find.text('Speaker'));
    expect(label.dy, greaterThan(mic.dy));
    expect(label.dy, lessThan(speaker.dy));
    expect(tester.widget<FgSwitch>(find.byType(FgSwitch)).on, isFalse);

    await tester.tap(find.text('Noise suppression'));
    await tester.pumpAndSettle();
    expect(state.prefs.noiseSuppression, isTrue);
    expect(tester.widget<FgSwitch>(find.byType(FgSwitch)).on, isTrue);

    await tester.tap(find.text('Noise suppression'));
    await tester.pumpAndSettle();
    expect(state.prefs.noiseSuppression, isFalse);
  });
}
