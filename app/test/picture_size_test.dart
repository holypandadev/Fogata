import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/account_move.dart';
import 'package:fogata/src/rust/frb_generated.dart';
import 'package:fogata/src/screens/friends.dart';
import 'package:fogata/src/screens/move_account.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/screens/welcome.dart';
import 'package:fogata/src/theme.dart';

import 'fake_room.dart';
import 'keychain_choice_test.dart' show core;

/// The width a PNG's header says, or null for what is no PNG.
int? _pngWidth(ByteData data) {
  if (data.lengthInBytes < 24 || data.getUint32(0) != 0x89504e47) return null;
  return data.getUint32(16);
}

/// Every picture of the app's own files drawn in [tester]'s window is decoded at about
/// the size it is drawn, not at its file's: the logo's 256 pixels kept for an 18-pixel
/// icon would hold 256 KB for nothing.
Future<void> _expectDecodedAsDrawn(WidgetTester tester) async {
  final dpr = tester.view.devicePixelRatio;
  var checked = 0;
  for (final element in find.byType(Image).evaluate()) {
    final image = element.widget as Image;
    final provider = image.image;
    if (provider is ResizeImage) {
      checked++;
      continue;
    }
    if (provider is! AssetImage) continue;
    final data = await tester.runAsync(
      () => rootBundle.load(provider.assetName),
    );
    final width = _pngWidth(data!);
    if (width == null) continue;
    final drawn = tester.getSize(find.byWidget(image)).width * dpr;
    expect(
      width,
      lessThanOrEqualTo(drawn * 2),
      reason: '${provider.assetName} is $width wide, drawn $drawn wide',
    );
    checked++;
  }
  expect(checked, greaterThan(0), reason: 'no picture was checked');
}

Future<AppState> _show(
  WidgetTester tester,
  Widget child, [
  AppState? state,
]) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 2;
  addTearDown(tester.view.reset);
  final shown = state ?? fakeRoom(people: 2);
  await tester.pumpWidget(
    AppScope(
      state: shown,
      child: MaterialApp(
        theme: Fg.darkTokens.themeData,
        home: Material(child: child),
      ),
    ),
  );
  await tester.pump(const Duration(milliseconds: 500));
  return shown;
}

void main() {
  setUpAll(() async {
    await loadFonts();
    RustLib.initMock(api: core);
  });

  testWidgets('the window, with its title bar', (tester) async {
    final state = await _show(tester, const Shell());
    await _expectDecodedAsDrawn(tester);
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('the welcome page', (tester) async {
    core.works = true;
    await _show(tester, const Welcome());
    await _expectDecodedAsDrawn(tester);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('an invitation\'s code', (tester) async {
    await _show(tester, const Center(child: QrBox(text: 'fogata://invite')));
    await _expectDecodedAsDrawn(tester);
  });

  testWidgets('the old computer after a move', (tester) async {
    final state = fakeRoom(people: 1)
      ..moveState = const UiMoveState(
        kind: 'moved',
        name: 'Marta',
        computer: 'Leo’s desktop',
        atMs: 0,
      );
    await _show(
      tester,
      const SizedBox(width: 1000, height: 800, child: MovedScreen()),
      state,
    );
    await _expectDecodedAsDrawn(tester);
    state.callState.call = null;
    await tester.pumpWidget(const SizedBox());
  });
}
