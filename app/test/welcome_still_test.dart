// The welcome page may show for a long time: once its bars have moved for a moment, it
// draws nothing more until something changes. Without a graphics card, a window redrawn
// 15 times a second kept several processors busy for as long as it showed.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/theme.dart';
import 'package:fogata/src/widgets.dart';

import 'fake_room.dart';

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

Future<void> _show(WidgetTester tester) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  // The name field's caret blinks as it does everywhere; it is not what is measured.
  EditableText.debugDeterministicCursor = true;
  addTearDown(() => EditableText.debugDeterministicCursor = false);
  await tester.pumpWidget(
    AppScope(
      state: fakeRoom(people: 1),
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: const Material(child: Welcome()),
      ),
    ),
  );
}

/// Whether a moment going by, with no frame drawn meanwhile, asks for one.
Future<bool> _asksForFrames(WidgetTester tester) async {
  await tester.binding.delayed(const Duration(milliseconds: 300));
  return tester.binding.hasScheduledFrame;
}

void main() {
  setUpAll(() => RustLib.initMock(api: _Core()));

  testWidgets('the welcome page stops drawing once its bars have moved a '
      'moment', (tester) async {
    await _show(tester);
    await tester.pump(const Duration(milliseconds: 500));
    // Moving at first: time going on asks for a frame.
    expect(await _asksForFrames(tester), isTrue);
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(seconds: 1));
    }
    expect(await _asksForFrames(tester), isFalse);
    await tester.pump(const Duration(seconds: 5));
    expect(await _asksForFrames(tester), isFalse);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('its bars hold still while a dialog covers it', (tester) async {
    await _show(tester);
    await tester.pump(const Duration(milliseconds: 200));
    final context = tester.element(find.byType(Welcome));
    showDialog<void>(
      context: context,
      builder: (_) => const AlertDialog(content: Text('Covered')),
    );
    // Within the time the bars would move, once the dialog is up and the last step of
    // the bars drawn.
    for (var i = 0; i < 3; i++) {
      await tester.pump(const Duration(milliseconds: 500));
    }
    expect(find.text('Covered'), findsOneWidget);
    expect(find.byType(SlowPulse, skipOffstage: false), findsNothing);
    expect(await _asksForFrames(tester), isFalse);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 5));
  });
}
