import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';
import 'theme_test.dart' show contrast;

/// A keychain that works, so the welcome page shows the card where the name is typed.
class _Core implements RustLibApi {
  @override
  Future<String?> crateApiAppKeychainProblem({required String dataDir}) async =>
      null;

  @override
  Future<String> crateApiAppDeviceFingerprint({
    required String dataDir,
  }) async => '7Q4M K2XR 9PLD 3VHC';

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName}');
}

void main() {
  setUpAll(() => RustLib.initMock(api: _Core()));

  // Before a name is typed, the card shows "Your name" where it will go, and in the
  // field: both read at WCAG's 4.5:1 over every surface the card can sit on.
  for (final look in Look.values.where((l) => l != Look.system)) {
    testWidgets('the name not typed yet reads in ${look.name}', (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'),
        (call) async => false,
      );
      tester.view.physicalSize = const Size(1440, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final c = look.tokens;
      await tester.pumpWidget(
        AppScope(
          state: fakeRoom(people: 1),
          child: MaterialApp(
            theme: c.themeData,
            home: const Material(child: Welcome()),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 500));
      final shown = find.text('Your name');
      expect(shown, findsNWidgets(2));
      for (final element in shown.evaluate()) {
        final paragraph = element.renderObject! as RenderParagraph;
        final color = paragraph.text.style!.color!;
        for (final MapEntry(key: surface, value: under) in {
          'bg': c.bg,
          'bg1': c.bg1,
          'bg2': c.bg2,
        }.entries) {
          expect(
            contrast(color, under),
            greaterThanOrEqualTo(4.5),
            reason: '"Your name" on $surface',
          );
        }
      }
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    });
  }
}
