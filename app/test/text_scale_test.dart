// The system's text size: followed up to twice the usual, where the pages still fit the
// window at its smallest; rows and controls that hold text grow with it instead of
// cutting it.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:fogata/src/app_state.dart';
import 'package:fogata/src/rust/api/direct.dart';
import 'package:fogata/src/rust/api/types.dart';
import 'package:fogata/src/screens/shell.dart';
import 'package:fogata/src/theme.dart';

import 'cut_text.dart';
import 'fake_room.dart';

const _sofia = UiPerson(
  id: 'sofia',
  name: 'Sofía',
  short: 'C07N·50S4',
  avatar: UiAvatar(preset: 2),
);

/// What overflowed or was cut while [state] showed in a window of [size] with text [scale] times
/// its size.
Future<List<String>> _overflows(
  WidgetTester tester,
  AppState state,
  Size size,
  double scale,
) async {
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('window_manager'),
    (call) async => false,
  );
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  addTearDown(tester.view.reset);
  final found = <String>[];
  final before = FlutterError.onError;
  // Only what overflows counts here: the pages also ask the core, which tests lack.
  FlutterError.onError = (details) {
    final text = details.toString();
    if (text.contains('overflowed')) found.add(text.split('\n').first);
  };
  try {
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          theme: Fg.darkTokens.themeData,
          home: const Material(child: Shell()),
          builder: appFrame,
        ),
      ),
    );
    for (var i = 0; i < 4; i++) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    found.addAll(cutText());
  } finally {
    FlutterError.onError = before;
  }
  await tester.pumpWidget(const SizedBox());
  await tester.pump(const Duration(seconds: 10));
  return found;
}

AppState _home() {
  final state = fakeRoom(people: 4)
    ..nav.page = Screen.home
    ..roomState.roomGroup = null
    ..nav.selectedGroup = null;
  state.direct.take([
    UiDirectChat(
      away: false,
      friend: _sofia,
      online: true,
      unread: 3,
      last: 'Are we still on for Friday?',
      lastMine: false,
      lastMs: 0,
      timerMs: 0,
    ),
  ]);
  return state;
}

AppState _direct() {
  final state = fakeRoom(people: 2)
    ..nav.page = Screen.direct
    ..roomState.roomGroup = null;
  final now = DateTime.now().millisecondsSinceEpoch;
  state.direct
    ..open = _sofia.id
    ..page = UiDirectPage(
      away: false,
      friend: _sofia,
      online: true,
      connected: true,
      items: [
        for (var i = 0; i < 6; i++)
          UiDirectItem(
            id: '$i',
            mine: i.isEven,
            atMs: now,
            text: 'Message number $i, a little longer than one word',
            files: const [],
            delivered: true,
            expiresMs: 0,
            pinned: i == 2,
          ),
      ],
      more: false,
      pinned: const [],
      timerMs: 0,
      usage: const UiDirectUsage(messages: 1, pictures: 0, videos: 0, files: 0),
      altered: false,
    );
  return state;
}

void main() {
  setUpAll(loadFonts);

  final pages = <String, AppState Function()>{
    'room': () => fakeRoom(people: 4, cameras: 1),
    'home': _home,
    'direct chat': _direct,
    'friends': () => fakeRoom(people: 3)..nav.page = Screen.friends,
    for (final section in SettingsSection.values)
      'settings ${section.name}': () =>
          fakeRoom(people: 2)..openSettings(section),
  };
  for (final size in const [Size(960, 640), Size(1280, 800)]) {
    for (final MapEntry(key: name, value: make) in pages.entries) {
      testWidgets('$name fits at twice the text size, $size', (tester) async {
        expect(await _overflows(tester, make(), size, 2), isEmpty);
      });
    }
  }

  testWidgets('text larger than twice the usual stops there', (tester) async {
    tester.platformDispatcher.textScaleFactorTestValue = 3;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    late double scaled;
    final state = AppState('/nonexistent/fogata-text-scale');
    await tester.pumpWidget(
      AppScope(
        state: state,
        child: MaterialApp(
          builder: appFrame,
          home: Builder(
            builder: (context) {
              scaled = MediaQuery.textScalerOf(context).scale(10);
              return const SizedBox();
            },
          ),
        ),
      ),
    );
    expect(scaled, 20);
    await tester.pumpWidget(const SizedBox());
    state.dispose();
  });
}
